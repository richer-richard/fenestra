//! The effect layer under the harness: `Cmd` tasks/futures/immediates and
//! `Sub::every` ticks run deterministically — same values, same order, no
//! wall clock, no races. This is the property that keeps effectful apps
//! verifiable in CI.

use std::time::Duration;

use fenestra_core::{App, Cmd, Element, Sub, by, col, text};
use fenestra_shell::Harness;

#[derive(Default)]
struct Fetcher {
    status: String,
    ticks: u32,
    subscribed: bool,
}

#[derive(Clone)]
enum Msg {
    Fetch,
    Got(String),
    Chained,
    FromFuture(u32),
    Tick,
    Subscribe,
    Unsubscribe,
}

impl App for Fetcher {
    type Msg = Msg;

    fn update(&mut self, _: Msg) {}

    fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Fetch => {
                self.status = "loading".into();
                Cmd::batch([
                    Cmd::task(|| Msg::Got("payload".into())),
                    Cmd::msg(Msg::Chained),
                ])
            }
            Msg::Got(s) => {
                self.status = format!("got {s}");
                Cmd::future(async { Msg::FromFuture(7) })
            }
            Msg::Chained => {
                self.status = format!("{}+chained", self.status);
                Cmd::none()
            }
            Msg::FromFuture(n) => {
                self.status = format!("{}+future{n}", self.status);
                Cmd::none()
            }
            Msg::Tick => {
                self.ticks += 1;
                Cmd::none()
            }
            Msg::Subscribe => {
                self.subscribed = true;
                Cmd::none()
            }
            Msg::Unsubscribe => {
                self.subscribed = false;
                Cmd::none()
            }
        }
    }

    fn subscriptions(&self) -> Vec<Sub<Msg>> {
        if self.subscribed {
            vec![Sub::every("tick", Duration::from_millis(300), || Msg::Tick)]
        } else {
            Vec::new()
        }
    }

    fn view(&self) -> Element<Msg> {
        col().p(8.0).children([
            text(format!("status: {}", self.status)).id("status"),
            text(format!("ticks: {}", self.ticks)).id("ticks"),
        ])
    }
}

fn harness() -> Harness<Fetcher> {
    Harness::new(
        Fetcher::default(),
        fenestra_core::Theme::light(),
        (300, 120),
    )
}

/// Immediate messages apply inline; deferred units wait for `run_effects`,
/// which resolves them synchronously — including a task whose result
/// queues a future.
#[test]
fn effects_resolve_deterministically() {
    let mut h = harness();
    h.update(Msg::Fetch);
    // The immediate Chained applied inline; the task is still pending.
    assert!(h.query(&by::label("status: loading+chained")).is_some());
    assert_eq!(h.pending_effects(), 1, "the task is queued, not run");

    let delivered = h.run_effects();
    // Task delivered Got, whose future then delivered FromFuture.
    assert_eq!(delivered, 2);
    assert_eq!(h.pending_effects(), 0);
    assert!(h.query(&by::label("status: got payload+future7")).is_some());
}

/// Subscription ticks fire on the deterministic clock: exactly
/// `elapsed / period` ticks, no more, and stopping the sub stops them.
#[test]
fn subscription_ticks_follow_the_pump_clock() {
    let mut h = harness();
    h.update(Msg::Subscribe);
    h.pump(1000.0);
    assert!(
        h.query(&by::label("ticks: 3")).is_some(),
        "300ms period over 1000ms = ticks at 300/600/900"
    );

    h.update(Msg::Unsubscribe);
    h.pump(1000.0);
    assert!(
        h.query(&by::label("ticks: 3")).is_some(),
        "a dropped subscription stops ticking"
    );

    // Re-subscribing schedules from now, not from the stale schedule.
    h.update(Msg::Subscribe);
    h.pump(350.0);
    assert!(h.query(&by::label("ticks: 4")).is_some());
}

/// Follow-up commands run in the order their messages applied (FIFO):
/// a batch delivering A then B runs A's follow-up chain before B's.
/// This pins the ordering contract shared by the live runners and the
/// harness (2026-07-24 review, finding: LIFO queue inverted sibling
/// follow-ups).
#[test]
fn follow_up_commands_run_in_message_order() {
    #[derive(Default)]
    struct Order {
        log: String,
    }
    #[derive(Clone)]
    enum OMsg {
        Kick,
        A,
        B,
        A2,
        B2,
    }
    impl App for Order {
        type Msg = OMsg;
        fn update(&mut self, _: OMsg) {}
        fn update_with(&mut self, msg: OMsg) -> Cmd<OMsg> {
            let (name, cmd) = match msg {
                OMsg::Kick => ("", Cmd::batch([Cmd::msg(OMsg::A), Cmd::msg(OMsg::B)])),
                OMsg::A => ("A", Cmd::msg(OMsg::A2)),
                OMsg::B => ("B", Cmd::msg(OMsg::B2)),
                OMsg::A2 => ("A2", Cmd::none()),
                OMsg::B2 => ("B2", Cmd::none()),
            };
            if !name.is_empty() {
                if !self.log.is_empty() {
                    self.log.push(',');
                }
                self.log.push_str(name);
            }
            cmd
        }
        fn view(&self) -> Element<OMsg> {
            text(format!("order: {}", self.log))
        }
    }
    let mut h = Harness::new(Order::default(), fenestra_core::Theme::light(), (300, 100));
    h.update(OMsg::Kick);
    assert!(
        h.query(&by::label("order: A,B,A2,B2")).is_some(),
        "follow-ups must run in message order (A's chain before B's)"
    );
}

/// `Cmd::map` lifts a child component's effect into the parent message
/// space, task output included.
#[test]
fn cmd_map_composes() {
    #[derive(Clone)]
    enum Child {
        Done(u8),
    }
    #[derive(Clone)]
    enum Parent {
        FromChild(u8),
    }
    let mapped: Cmd<Parent> =
        Cmd::task(|| Child::Done(3)).map(|Child::Done(n)| Parent::FromChild(n));
    let mut units = Vec::new();
    mapped.run(&mut |_| panic!("task is deferred"), &mut |u| units.push(u));
    let Parent::FromChild(n) = units.remove(0).block();
    assert_eq!(n, 3);
}
