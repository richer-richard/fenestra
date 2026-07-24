//! The Elm-shaped application contract.

use crate::cmd::{Cmd, Sub};
use crate::element::Element;
use crate::menu::MenuSpec;
use crate::proxy::Proxy;
use crate::theme::Theme;

/// A secondary window the app wants open: presence in
/// [`App::windows`]'s list opens it, removal closes it (exactly like
/// modal state). The OS close button emits `on_close` — remove the desc
/// in `update` to actually close.
#[derive(Debug, Clone)]
pub struct WindowDesc<Msg> {
    /// Stable identity: per-window state (focus, scroll, editors) keys
    /// off it, and [`App::view_for`] receives it.
    pub key: String,
    /// Window title (live-updated when it changes).
    pub title: String,
    /// Inner size in logical pixels, applied at open.
    pub size: (f64, f64),
    /// Emitted when the user closes the window via the OS.
    pub on_close: Msg,
}

impl<Msg> WindowDesc<Msg> {
    /// A window description.
    pub fn new(
        key: impl Into<String>,
        title: impl Into<String>,
        size: (f64, f64),
        on_close: Msg,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            size,
            on_close,
        }
    }
}

/// An application: state, a pure view of it, and a message-driven update.
///
/// ```
/// use fenestra_core::*;
///
/// struct Counter {
///     n: i64,
/// }
///
/// #[derive(Clone)]
/// enum Msg {
///     Inc,
///     Dec,
/// }
///
/// impl App for Counter {
///     type Msg = Msg;
///
///     fn update(&mut self, msg: Msg) {
///         match msg {
///             Msg::Inc => self.n += 1,
///             Msg::Dec => self.n -= 1,
///         }
///     }
///
///     fn view(&self) -> Element<Msg> {
///         col().items_center().children([text(self.n.to_string())])
///     }
/// }
/// ```
pub trait App {
    /// The message type carried by handlers. Cloned on dispatch.
    type Msg: Clone + 'static;

    /// Called once by the runner before the first frame, with a [`Proxy`]
    /// that delivers messages into [`Self::update`] from outside the view:
    /// background threads, timers, IO completion. The default does nothing.
    /// Store the proxy (or move clones into threads) to send later.
    fn init(&mut self, proxy: Proxy<Self::Msg>) {
        let _ = proxy;
    }

    /// Applies one message to the state.
    fn update(&mut self, msg: Self::Msg);

    /// Applies one message and returns the effect to run — the runner (and
    /// the test harness) call *this*, which by default delegates to
    /// [`Self::update`] with no effect. Effectful apps override
    /// `update_with` and leave `update` as an empty body:
    ///
    /// ```
    /// use fenestra_core::*;
    ///
    /// struct Fetcher {
    ///     status: String,
    /// }
    ///
    /// #[derive(Clone)]
    /// enum Msg {
    ///     Fetch,
    ///     Got(String),
    /// }
    ///
    /// impl App for Fetcher {
    ///     type Msg = Msg;
    ///     fn update(&mut self, _: Msg) {}
    ///     fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
    ///         match msg {
    ///             Msg::Fetch => {
    ///                 self.status = "loading".into();
    ///                 Cmd::task(|| Msg::Got("done".into()))
    ///             }
    ///             Msg::Got(s) => {
    ///                 self.status = s;
    ///                 Cmd::none()
    ///             }
    ///         }
    ///     }
    ///     fn view(&self) -> Element<Msg> {
    ///         text(&self.status)
    ///     }
    /// }
    /// ```
    fn update_with(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        self.update(msg);
        Cmd::none()
    }

    /// The effect to run once at startup, after [`Self::init`] (initial
    /// data loads). The default is none.
    fn init_cmd(&mut self) -> Cmd<Self::Msg> {
        Cmd::none()
    }

    /// Recurring effects the app wants while its state says so (timer
    /// ticks), reconciled by key after every update exactly like
    /// [`Self::windows`]: new keys start, missing keys stop. The default is
    /// none. (Native only; the web runner ignores subscriptions.)
    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        Vec::new()
    }

    /// The native menu bar, declared from state and reconciled after every
    /// update (rebuilt only when its structure changes); chosen items come
    /// back as messages. `None` (the default) leaves the platform menu
    /// untouched. Attaches on macOS; other platforms keep the kit's
    /// in-window `menubar` (see [`MenuSpec`]'s platform notes).
    fn menu(&self) -> Option<MenuSpec<Self::Msg>> {
        None
    }

    /// Builds the view. Pure and cheap: called on every redraw, the whole
    /// tree is rebuilt, laid out, and repainted (no diffing).
    fn view(&self) -> Element<Self::Msg>;

    /// The theme to render with. Override to store the theme in app state
    /// (e.g. for a light/dark toggle).
    fn theme(&self) -> Theme {
        Theme::light()
    }

    /// Secondary windows to keep open, reconciled after every update:
    /// new keys open, missing keys close, changed titles apply. The
    /// default is none — single-window apps never see this API.
    /// (Native only; the web runner ignores secondary windows.)
    fn windows(&self) -> Vec<WindowDesc<Self::Msg>> {
        Vec::new()
    }

    /// The view for one window: `view_for("main")` is the main window,
    /// other keys come from [`Self::windows`]. Defaults to [`Self::view`]
    /// everywhere, so single-window apps only implement `view`.
    fn view_for(&self, key: &str) -> Element<Self::Msg> {
        let _ = key;
        self.view()
    }

    /// The view for one window at a given available size (logical px: the
    /// content size the frame is laid out at). Override to switch layout on
    /// window-size breakpoints (see [`Breakpoints`](crate::Breakpoints)).
    /// Defaults to [`Self::view_for`] (size ignored), so apps opt in. For a
    /// *container's* own size, reach for [`responsive`](crate::responsive)
    /// instead.
    fn view_at(&self, key: &str, size: (f32, f32)) -> Element<Self::Msg> {
        let _ = size;
        self.view_for(key)
    }

    /// The theme for one window; defaults to [`Self::theme`] everywhere.
    /// Override for per-window theming (e.g. a dark inspector next to a
    /// light main window). The windowed runner consults it per window;
    /// the test harness keeps its single explicit theme for determinism.
    fn theme_for(&self, key: &str) -> Theme {
        let _ = key;
        self.theme()
    }
}

/// A mutable borrow of an app is itself an app: harnesses can drive an
/// app the caller still owns (and inspects afterwards).
impl<A: App> App for &mut A {
    type Msg = A::Msg;

    fn init(&mut self, proxy: Proxy<Self::Msg>) {
        (**self).init(proxy);
    }

    fn update(&mut self, msg: Self::Msg) {
        (**self).update(msg);
    }

    fn update_with(&mut self, msg: Self::Msg) -> Cmd<Self::Msg> {
        (**self).update_with(msg)
    }

    fn init_cmd(&mut self) -> Cmd<Self::Msg> {
        (**self).init_cmd()
    }

    fn subscriptions(&self) -> Vec<Sub<Self::Msg>> {
        (**self).subscriptions()
    }

    fn menu(&self) -> Option<MenuSpec<Self::Msg>> {
        (**self).menu()
    }

    fn view(&self) -> Element<Self::Msg> {
        (**self).view()
    }

    fn theme(&self) -> Theme {
        (**self).theme()
    }

    fn windows(&self) -> Vec<WindowDesc<Self::Msg>> {
        (**self).windows()
    }

    fn view_for(&self, key: &str) -> Element<Self::Msg> {
        (**self).view_for(key)
    }

    fn view_at(&self, key: &str, size: (f32, f32)) -> Element<Self::Msg> {
        (**self).view_at(key, size)
    }

    fn theme_for(&self, key: &str) -> Theme {
        (**self).theme_for(key)
    }
}

/// The key [`App::view_for`] receives for the main window.
pub const MAIN_WINDOW: &str = "main";
