//! Effects as values: the async story for [`App`](crate::App).
//!
//! [`update_with`](crate::App::update_with) returns a [`Cmd`] describing
//! what should happen *outside* the pure state machine — a blocking task on
//! a worker thread, a future, a follow-up message — and the runner executes
//! it, delivering the resulting message back through `update_with`. The
//! test harness executes the same values synchronously under its
//! deterministic clock, so effectful apps stay verifiable in CI.
//!
//! [`Sub`] is the recurring counterpart: [`App::subscriptions`] declares
//! what the app wants to keep receiving (timer ticks), reconciled by key
//! after every update exactly like secondary windows.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// A boxed future producing one message.
pub type CmdFuture<Msg> = Pin<Box<dyn Future<Output = Msg> + Send>>;

/// An effect description returned by [`App::update_with`](crate::App::update_with).
/// Values, not actions: nothing runs until the runner (or the test
/// harness) executes it.
pub struct Cmd<Msg> {
    pub(crate) kind: CmdKind<Msg>,
}

pub(crate) enum CmdKind<Msg> {
    None,
    /// Deliver this message on the next loop turn.
    Msg(Msg),
    /// Run on a worker thread (blocking allowed: HTTP, file IO, heavy
    /// compute); the returned message is delivered when it finishes.
    Task(Box<dyn FnOnce() -> Msg + Send>),
    /// Drive this future to completion off the UI thread. Executed with a
    /// minimal block-on executor, so it must be runtime-agnostic (no
    /// tokio-bound IO types; for those, own the runtime and send results
    /// through the [`Proxy`](crate::Proxy) instead).
    Future(CmdFuture<Msg>),
    /// Run several effects concurrently.
    Batch(Vec<Cmd<Msg>>),
}

impl<Msg> Cmd<Msg> {
    /// No effect — what plain state updates return.
    #[must_use]
    pub fn none() -> Self {
        Self {
            kind: CmdKind::None,
        }
    }

    /// Delivers `msg` on the next loop turn (a self-transition without
    /// recursing inside `update`).
    #[must_use]
    pub fn msg(msg: Msg) -> Self {
        Self {
            kind: CmdKind::Msg(msg),
        }
    }

    /// Runs `f` on a worker thread — blocking is fine (HTTP with a blocking
    /// client, file IO, heavy compute) — and delivers its return value as a
    /// message.
    #[must_use]
    pub fn task(f: impl FnOnce() -> Msg + Send + 'static) -> Self {
        Self {
            kind: CmdKind::Task(Box::new(f)),
        }
    }

    /// Drives a runtime-agnostic future off the UI thread and delivers its
    /// output as a message. See [`CmdKind::Future`]'s caveat: futures that
    /// need a specific reactor (tokio IO) should run on their own runtime,
    /// reporting back through the [`Proxy`](crate::Proxy).
    #[must_use]
    pub fn future(fut: impl Future<Output = Msg> + Send + 'static) -> Self {
        Self {
            kind: CmdKind::Future(Box::pin(fut)),
        }
    }

    /// Runs several effects concurrently (order of delivery is by
    /// completion, not position).
    #[must_use]
    pub fn batch(cmds: impl IntoIterator<Item = Cmd<Msg>>) -> Self {
        let mut flat: Vec<Cmd<Msg>> = cmds
            .into_iter()
            .filter(|c| !matches!(c.kind, CmdKind::None))
            .collect();
        match flat.len() {
            0 => Self::none(),
            1 => flat.remove(0),
            _ => Self {
                kind: CmdKind::Batch(flat),
            },
        }
    }

    /// Converts the produced message with `f` — the composition tool that
    /// lets a component's `Cmd<ChildMsg>` drop into a parent, mirroring
    /// [`Element::map`](crate::Element::map).
    #[must_use]
    pub fn map<B: 'static>(self, f: impl Fn(Msg) -> B + Send + Sync + Clone + 'static) -> Cmd<B>
    where
        Msg: 'static,
    {
        let kind = match self.kind {
            CmdKind::None => CmdKind::None,
            CmdKind::Msg(m) => CmdKind::Msg(f(m)),
            CmdKind::Task(t) => CmdKind::Task(Box::new(move || f(t()))),
            CmdKind::Future(fut) => CmdKind::Future(Box::pin(async move { f(fut.await) })),
            CmdKind::Batch(cmds) => {
                CmdKind::Batch(cmds.into_iter().map(|c| c.map(f.clone())).collect())
            }
        };
        Cmd { kind }
    }

    /// Whether this is [`Cmd::none`] (nothing to execute).
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self.kind, CmdKind::None)
    }

    /// Executes the description's *shape*: immediate messages go to `now`,
    /// each deferred unit (task or future) goes to `spawn`. Runners decide
    /// the threading; the test harness runs units synchronously via
    /// [`CmdUnit::block`].
    pub fn run(self, now: &mut impl FnMut(Msg), spawn: &mut impl FnMut(CmdUnit<Msg>)) {
        match self.kind {
            CmdKind::None => {}
            CmdKind::Msg(m) => now(m),
            CmdKind::Task(t) => spawn(CmdUnit::Task(t)),
            CmdKind::Future(f) => spawn(CmdUnit::Future(f)),
            CmdKind::Batch(parts) => {
                for c in parts {
                    c.run(now, spawn);
                }
            }
        }
    }
}

/// One deferred unit of work inside a [`Cmd`], produced by [`Cmd::run`].
pub enum CmdUnit<Msg> {
    /// A blocking-allowed closure.
    Task(Box<dyn FnOnce() -> Msg + Send>),
    /// A runtime-agnostic future.
    Future(CmdFuture<Msg>),
}

impl<Msg> CmdUnit<Msg> {
    /// Runs the unit to completion on the current thread — what the test
    /// harness uses for determinism, and what runners call from worker
    /// threads. Futures poll under a minimal park-based waker, so they must
    /// be runtime-agnostic (see [`Cmd::future`]).
    #[must_use]
    pub fn block(self) -> Msg {
        match self {
            Self::Task(f) => f(),
            Self::Future(mut fut) => {
                struct ThreadWaker(std::thread::Thread);
                impl std::task::Wake for ThreadWaker {
                    fn wake(self: Arc<Self>) {
                        self.0.unpark();
                    }
                }
                let waker = std::task::Waker::from(Arc::new(ThreadWaker(std::thread::current())));
                let mut cx = std::task::Context::from_waker(&waker);
                loop {
                    match fut.as_mut().poll(&mut cx) {
                        std::task::Poll::Ready(m) => return m,
                        std::task::Poll::Pending => std::thread::park(),
                    }
                }
            }
        }
    }
}

impl<Msg> Default for Cmd<Msg> {
    fn default() -> Self {
        Self::none()
    }
}

impl<Msg> std::fmt::Debug for Cmd<Msg> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match &self.kind {
            CmdKind::None => "Cmd::none",
            CmdKind::Msg(_) => "Cmd::msg",
            CmdKind::Task(_) => "Cmd::task",
            CmdKind::Future(_) => "Cmd::future",
            CmdKind::Batch(_) => "Cmd::batch",
        };
        f.write_str(name)
    }
}

/// A recurring effect the app wants while its state says so, declared from
/// [`App::subscriptions`](crate::App::subscriptions) and reconciled by
/// `key` after every update: new keys start, missing keys stop, surviving
/// keys keep running (a changed period under the same key restarts it).
pub struct Sub<Msg> {
    /// Stable identity, like a [`WindowDesc`](crate::WindowDesc) key.
    pub(crate) key: String,
    pub(crate) kind: SubKind<Msg>,
}

pub(crate) enum SubKind<Msg> {
    /// Deliver `make()` every `period`.
    Every {
        period: Duration,
        make: Arc<dyn Fn() -> Msg + Send + Sync>,
    },
}

impl<Msg> Sub<Msg> {
    /// Delivers `make()` every `period`, keyed by `key`. Periods clamp to
    /// at least one millisecond (a zero period would spin the loop).
    #[must_use]
    pub fn every(
        key: impl Into<String>,
        period: Duration,
        make: impl Fn() -> Msg + Send + Sync + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            kind: SubKind::Every {
                period: period.max(Duration::from_millis(1)),
                make: Arc::new(make),
            },
        }
    }

    /// The subscription's stable key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The tick period.
    #[must_use]
    pub fn period(&self) -> Duration {
        match &self.kind {
            SubKind::Every { period, .. } => *period,
        }
    }

    /// Produces one tick message (used by runners and the test harness).
    #[must_use]
    pub fn tick(&self) -> Msg {
        match &self.kind {
            SubKind::Every { make, .. } => make(),
        }
    }
}

impl<Msg> std::fmt::Debug for Sub<Msg> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SubKind::Every { period, .. } => f
                .debug_struct("Sub::every")
                .field("key", &self.key)
                .field("period", period)
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_flattens_nones() {
        let c: Cmd<u32> = Cmd::batch([Cmd::none(), Cmd::none()]);
        assert!(c.is_none());
        let c: Cmd<u32> = Cmd::batch([Cmd::none(), Cmd::msg(1)]);
        assert!(matches!(c.kind, CmdKind::Msg(1)));
    }

    #[test]
    fn map_reaches_every_variant() {
        let c = Cmd::batch([Cmd::msg(1), Cmd::task(|| 2)]).map(|n: u32| n * 10);
        let CmdKind::Batch(parts) = c.kind else {
            panic!("expected a batch");
        };
        assert!(matches!(parts[0].kind, CmdKind::Msg(10)));
        let CmdKind::Task(task) = parts.into_iter().nth(1).expect("two parts").kind else {
            panic!("expected a task");
        };
        assert_eq!(task(), 20, "the map must wrap the task's output");
    }

    #[test]
    fn sub_period_clamps_to_a_millisecond() {
        let s = Sub::every("tick", Duration::ZERO, || 1_u32);
        assert_eq!(s.period(), Duration::from_millis(1));
        assert_eq!(s.tick(), 1);
    }
}
