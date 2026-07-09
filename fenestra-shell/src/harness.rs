//! The verification harness: drive an [`App`] headlessly through
//! semantic queries instead of coordinates, and assert at three levels —
//! pixels, accessibility tree, and emitted messages.
//!
//! ```no_run
//! use fenestra_core::{App, by};
//! use fenestra_shell::Harness;
//! # struct Todo; #[derive(Clone)] enum Msg { Add }
//! # impl App for Todo { type Msg = Msg; fn update(&mut self, _: Msg) {}
//! #   fn view(&self) -> fenestra_core::Element<Msg> { fenestra_core::col() } }
//! let mut h = Harness::new(Todo, fenestra_core::Theme::light(), (480, 320));
//! h.click(&by::label("Add"));            // find like a user, not by (x, y)
//! h.type_text("buy milk");
//! assert!(h.query(&by::label("buy milk")).is_some());
//! let _png = h.render();                 // pixels only when asked
//! ```
//!
//! Determinism: scale 1.0, reduced motion, embedded fonts, and an
//! explicit clock — animations only advance when [`Harness::pump`] is
//! called. Nothing is painted unless [`Harness::render`] is called, so
//! structural tests stay fast. [`Harness::film`] captures a whole sequence
//! of renders across the clock, so an agent can watch a transition play
//! instead of only ever seeing frozen frames — see its docs for how that
//! squares with the reduced-motion default above.

use std::sync::{Arc, Mutex, PoisonError};

use std::collections::HashMap;

use fenestra_core::{
    AccessNode, App, Element, Frame, FrameState, InputEvent, KeyInput, MAIN_WINDOW, Proxy, Query,
    Theme, build_frame, dispatch,
};
use image::RgbaImage;

use crate::element_render::with_fonts;
use crate::with_headless;

/// Hard ceiling on `frames` in [`Harness::film`]: a filmstrip is meant for
/// an agent to review in one sitting, and every frame is a full GPU render —
/// this many already takes seconds and produces a strip nobody reviews at a
/// glance. Clamp-over-panic: a hostile or mistaken huge request degrades to
/// this ceiling instead of hanging or exhausting memory rendering it.
pub const MAX_FILM_FRAMES: usize = 64;

/// Hard ceiling on `interval_ms` in [`Harness::film`]: a span this long turns
/// "watch a transition play" into unrelated snapshots minutes apart. Chain
/// several `film` calls (or `pump` between them) to cover a longer timeline
/// at a sensible cadence.
pub const MAX_FILM_INTERVAL_MS: u64 = 60_000;

/// One headless window: its own retained state, view, and frame —
/// exactly like the windowed runner keeps per window.
struct WindowSlot<Msg> {
    state: FrameState,
    view: Element<Msg>,
    frame: Frame,
    logical: (f32, f32),
    size: (u32, u32),
}

/// A headless app under test. See the module docs for the model.
pub struct Harness<A: App> {
    app: A,
    theme: Theme,
    /// Deterministic clock in seconds, advanced only by [`Self::pump`].
    clock: f64,
    /// Messages emitted by handlers since the last [`Self::take_messages`].
    msgs: Vec<A::Msg>,
    pending: Arc<Mutex<Vec<A::Msg>>>,
    /// Open windows by key; reconciled against [`App::windows`] after
    /// every update, exactly like the windowed runner.
    slots: HashMap<String, WindowSlot<A::Msg>>,
    /// Animations snap by default (deterministic); motion tests opt in.
    reduced_motion: bool,
    /// The window verbs and queries currently target.
    active: String,
}

impl<A: App> Harness<A>
where
    A::Msg: Send,
{
    /// Builds the first frame. [`App::init`] runs with a collecting
    /// [`Proxy`]; proxied messages drain at every rebuild (after each
    /// input, [`Self::pump`], or [`Self::update`]).
    ///
    /// # Panics
    /// If no compute-capable GPU adapter exists.
    pub fn new(mut app: A, theme: Theme, size: (u32, u32)) -> Self {
        let size =
            with_headless(|h| h.clamp_size(size.0, size.1)).expect("headless renderer unavailable");
        let pending: Arc<Mutex<Vec<A::Msg>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&pending);
        app.init(Proxy::new(move |msg| {
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(msg);
        }));
        Self::drain(&mut app, &pending);
        let mut harness = Self {
            app,
            theme,
            clock: 0.0,
            msgs: Vec::new(),
            pending,
            slots: HashMap::new(),
            active: MAIN_WINDOW.to_owned(),
            reduced_motion: true,
        };
        harness.slots.insert(
            MAIN_WINDOW.to_owned(),
            Self::new_slot(&harness.app, &harness.theme, MAIN_WINDOW, size, 0.0, true),
        );
        harness.rebuild();
        harness
    }

    fn new_slot(
        app: &A,
        theme: &Theme,
        key: &str,
        size: (u32, u32),
        clock: f64,
        reduced_motion: bool,
    ) -> WindowSlot<A::Msg> {
        let size =
            with_headless(|h| h.clamp_size(size.0, size.1)).expect("headless renderer unavailable");
        let mut state = FrameState::new();
        state.reduced_motion = reduced_motion;
        state.tick(clock);
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let logical = (size.0 as f32, size.1 as f32);
        let view = app.view_at(key, logical);
        let frame = with_fonts(|fonts| build_frame(&view, theme, fonts, &mut state, logical, 1.0));
        WindowSlot {
            state,
            view,
            frame,
            logical,
            size,
        }
    }

    fn drain(app: &mut A, pending: &Mutex<Vec<A::Msg>>) {
        let msgs = std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
        for msg in msgs {
            app.update(msg);
        }
    }

    /// Rebuilds every window from current app state (proxied messages
    /// drain first) and reconciles the declared window set: new keys
    /// open, missing keys close (the active window falls back to main).
    /// Runs automatically after every input; call it yourself only
    /// after mutating via [`Self::app_mut`].
    pub fn rebuild(&mut self) {
        Self::drain(&mut self.app, &self.pending);
        let descs = self.app.windows();
        self.slots
            .retain(|key, _| key == MAIN_WINDOW || descs.iter().any(|d| &d.key == key));
        for desc in &descs {
            if !self.slots.contains_key(&desc.key) {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "logical window sizes are small positive numbers"
                )]
                let size = (desc.size.0.max(1.0) as u32, desc.size.1.max(1.0) as u32);
                let slot = Self::new_slot(
                    &self.app,
                    &self.theme,
                    &desc.key,
                    size,
                    self.clock,
                    self.reduced_motion,
                );
                self.slots.insert(desc.key.clone(), slot);
            }
        }
        if !self.slots.contains_key(&self.active) {
            self.active = MAIN_WINDOW.to_owned();
        }
        let keys: Vec<String> = self.slots.keys().cloned().collect();
        for key in keys {
            let slot = self.slots.get_mut(&key).expect("slot exists");
            slot.view = self.app.view_at(&key, slot.logical);
            slot.state.tick(self.clock);
            slot.frame = with_fonts(|fonts| {
                build_frame(
                    &slot.view,
                    &self.theme,
                    fonts,
                    &mut slot.state,
                    slot.logical,
                    1.0,
                )
            });
        }
    }

    fn slot(&self) -> &WindowSlot<A::Msg> {
        self.slots.get(&self.active).expect("active slot exists")
    }

    /// Enables or disables real animation. The harness defaults to
    /// reduced motion (everything snaps — deterministic pixels); motion
    /// tests opt into physics and drive it with [`Self::pump`].
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
        for slot in self.slots.values_mut() {
            slot.state.reduced_motion = reduced;
        }
        self.rebuild();
    }

    /// Switches which window the verbs and queries target. Open windows
    /// come from [`App::windows`]; [`MAIN_WINDOW`] is always open.
    ///
    /// # Panics
    /// If no open window has this key (the message lists the open ones).
    pub fn activate_window(&mut self, key: &str) {
        assert!(
            self.slots.contains_key(key),
            "no open window {key:?}; open windows: {:?}",
            self.window_keys()
        );
        self.active = key.to_owned();
    }

    /// Resizes one window: clamps to the renderer's limits, updates the slot's
    /// pixel and logical size, and rebuilds its frame via [`App::view_at`] at
    /// the new size — the headless analogue of dragging a window edge, and how
    /// `view_at` window breakpoints and
    /// [`responsive`](fenestra_core::responsive) container queries are driven.
    /// Other windows are untouched.
    ///
    /// # Panics
    /// If no open window has this key.
    pub fn resize(&mut self, key: &str, width: u32, height: u32) {
        assert!(
            self.slots.contains_key(key),
            "no open window {key:?}; open windows: {:?}",
            self.window_keys()
        );
        let size =
            with_headless(|h| h.clamp_size(width, height)).expect("headless renderer unavailable");
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let logical = (size.0 as f32, size.1 as f32);
        let view = self.app.view_at(key, logical);
        let slot = self.slots.get_mut(key).expect("checked above");
        slot.size = size;
        slot.logical = logical;
        slot.view = view;
        slot.state.tick(self.clock);
        slot.frame = with_fonts(|fonts| {
            build_frame(
                &slot.view,
                &self.theme,
                fonts,
                &mut slot.state,
                logical,
                1.0,
            )
        });
    }

    /// The keys of every open window, sorted (main first).
    pub fn window_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.slots.keys().cloned().collect();
        keys.sort_by_key(|k| (k != MAIN_WINDOW, k.clone()));
        keys
    }

    /// Dispatches one raw input event against the active window's
    /// current frame, logs and applies the emitted messages, and
    /// rebuilds (which also reconciles the window set).
    pub fn input(&mut self, event: InputEvent) {
        let slot = self
            .slots
            .get_mut(&self.active)
            .expect("active slot exists");
        let result =
            with_fonts(|fonts| dispatch(&slot.view, &slot.frame, &mut slot.state, fonts, event));
        for msg in result.msgs {
            self.msgs.push(msg.clone());
            self.app.update(msg);
        }
        self.rebuild();
    }

    fn center(&self, q: &Query) -> (f32, f32) {
        let node = self.slot().frame.get(q);
        let c = node.rect.center();
        #[expect(clippy::cast_possible_truncation, reason = "logical px fit in f32")]
        (c.x as f32, c.y as f32)
    }

    /// Moves the pointer to the center of the matched node.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn hover(&mut self, q: &Query) {
        let (x, y) = self.center(q);
        self.input(InputEvent::PointerMove { x, y });
    }

    /// Clicks (press + release) the center of the matched node.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn click(&mut self, q: &Query) {
        self.hover(q);
        self.input(InputEvent::PointerDown);
        self.input(InputEvent::PointerUp);
    }

    /// Right-clicks the center of the matched node.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn right_click(&mut self, q: &Query) {
        self.hover(q);
        self.input(InputEvent::RightDown);
        self.input(InputEvent::RightUp);
    }

    /// Double-clicks the matched node (two clicks inside the
    /// double-click window — the harness clock does not advance).
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn double_click(&mut self, q: &Query) {
        self.click(q);
        self.click(q);
    }

    /// Triple-clicks the matched node (text inputs select the line).
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn triple_click(&mut self, q: &Query) {
        self.click(q);
        self.click(q);
        self.click(q);
    }

    /// Clicks with Shift held (text inputs extend the selection from
    /// the caret to the click point).
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn shift_click(&mut self, q: &Query) {
        self.input(InputEvent::Modifiers {
            shift: true,
            ctrl: false,
            alt: false,
            meta: false,
        });
        self.click(q);
        self.input(InputEvent::Modifiers {
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        });
    }

    /// Commits text to the focused element (like typing or IME commit).
    pub fn type_text(&mut self, text: impl Into<String>) {
        self.input(InputEvent::Text(text.into()));
    }

    /// Presses one key.
    pub fn key(&mut self, key: KeyInput) {
        self.input(InputEvent::Key(key));
    }

    /// Focuses the next focusable element (Tab).
    pub fn tab(&mut self) {
        self.input(InputEvent::Tab);
    }

    /// Focuses the previous focusable element (Shift-Tab).
    pub fn shift_tab(&mut self) {
        self.input(InputEvent::ShiftTab);
    }

    /// Focuses the matched node directly (what assistive technology's
    /// Focus action does). Prefer [`Self::tab`] to test the real path.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn focus(&mut self, q: &Query) {
        let slot = self
            .slots
            .get_mut(&self.active)
            .expect("active slot exists");
        let id = slot.frame.get(q).id;
        slot.state.set_focus(Some(id));
        self.rebuild();
    }

    /// Drags from one node to another: press on `from`, move to `to`
    /// (recomputed after the press, in case layout shifted), release.
    ///
    /// # Panics
    /// If either query matches zero or several nodes.
    pub fn drag(&mut self, from: &Query, to: &Query) {
        self.hover(from);
        self.input(InputEvent::PointerDown);
        let (x, y) = self.center(to);
        self.input(InputEvent::PointerMove { x, y });
        self.input(InputEvent::PointerUp);
    }

    /// Drops an OS file onto the matched node.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn drop_file(&mut self, q: &Query, path: impl Into<std::path::PathBuf>) {
        self.hover(q);
        self.input(InputEvent::FileDrop(path.into()));
    }

    /// Scrolls the wheel over the matched node (positive `dy` moves
    /// content down, winit convention).
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn wheel(&mut self, q: &Query, dy: f32) {
        self.hover(q);
        self.input(InputEvent::Wheel { dx: 0.0, dy });
    }

    /// Scrolls the wheel on both axes over the matched node (positive `dx`
    /// moves content right, positive `dy` moves content down).
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn wheel_xy(&mut self, q: &Query, dx: f32, dy: f32) {
        self.hover(q);
        self.input(InputEvent::Wheel { dx, dy });
    }

    /// Advances the deterministic clock by `ms` milliseconds and
    /// rebuilds — animations and timers move exactly this far.
    pub fn pump(&mut self, ms: f64) {
        self.clock += ms / 1000.0;
        self.rebuild();
    }

    /// Applies one message directly (as a proxy or window event would)
    /// and rebuilds. Not logged in [`Self::take_messages`].
    pub fn update(&mut self, msg: A::Msg) {
        self.app.update(msg);
        self.rebuild();
    }

    /// The single matching node; panics (with the accessibility tree in
    /// the message) on zero or several matches.
    ///
    /// # Panics
    /// If the query matches zero or several nodes.
    pub fn get(&self, q: &Query) -> AccessNode {
        self.slot().frame.get(q)
    }

    /// The single matching node, or `None`. Use to assert absence.
    ///
    /// # Panics
    /// If the query matches several nodes.
    pub fn query(&self, q: &Query) -> Option<AccessNode> {
        self.slot().frame.query(q)
    }

    /// Every matching node in tree order.
    pub fn get_all(&self, q: &Query) -> Vec<AccessNode> {
        self.slot().frame.get_all(q)
    }

    /// Messages emitted by handlers since the last call (the Elm-level
    /// assertion: *what the UI said*, independent of state effects).
    /// Proxied and [`Self::update`] messages are inputs, not logged.
    pub fn take_messages(&mut self) -> Vec<A::Msg> {
        std::mem::take(&mut self.msgs)
    }

    /// The active window's current frame, for direct queries and
    /// `access_yaml()`.
    pub fn frame(&self) -> &Frame {
        &self.slot().frame
    }

    /// The app under test.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Mutable access to the app; call [`Self::rebuild`] afterwards.
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// Renders the active window to pixels. Mid-test captures are fine —
    /// the frame is not consumed.
    ///
    /// # Panics
    /// If rendering fails.
    pub fn render(&mut self) -> RgbaImage {
        let key = self.active.clone();
        self.render_window(&key)
    }

    /// Renders any open window to pixels.
    ///
    /// # Panics
    /// If no open window has this key, or rendering fails.
    pub fn render_window(&mut self, key: &str) -> RgbaImage {
        assert!(
            self.slots.contains_key(key),
            "no open window {key:?}; open windows: {:?}",
            self.window_keys()
        );
        let bg = self.theme.bg;
        let slot = self.slots.get_mut(key).expect("checked above");
        // Two-pass planner (fonts → headless lock order) so frosted glass blurs;
        // glass-free frames fast-path to a single pass.
        with_fonts(|fonts| {
            with_headless(|h| {
                h.render_plan(
                    &slot.frame,
                    fonts,
                    &mut slot.state,
                    slot.size.0,
                    slot.size.1,
                    bg,
                )
            })
            .expect("headless renderer unavailable")
        })
        .expect("headless render failed")
    }

    /// Captures `frames` renders of the active window, `interval_ms` apart on
    /// the deterministic clock: the first frame is the window exactly as it
    /// stands now, then [`Self::pump`] and [`Self::render`] repeat — so
    /// `film(3, 100)` returns the states at +0ms, +100ms, +200ms.
    ///
    /// [`Self::new`] defaults every harness to reduced motion, the same
    /// default every other verification path relies on so single-shot
    /// goldens stay stable — under it every transition snaps to its target
    /// immediately, so a filmstrip captured without changing that is `frames`
    /// copies of the same pixels. Call
    /// [`Self::set_reduced_motion`]`(false)` first to see real motion play;
    /// determinism still holds, because it comes from the clock (advanced
    /// only by [`Self::pump`]), never from suppressing animation.
    ///
    /// `frames` is floored at 1 and clamped to [`MAX_FILM_FRAMES`];
    /// `interval_ms` is clamped to [`MAX_FILM_INTERVAL_MS`] (see their docs).
    ///
    /// # Panics
    /// If rendering fails (see [`Self::render`]).
    pub fn film(&mut self, frames: usize, interval_ms: u64) -> Vec<RgbaImage> {
        let frames = frames.clamp(1, MAX_FILM_FRAMES);
        let interval_ms = interval_ms.min(MAX_FILM_INTERVAL_MS);
        let mut out = Vec::with_capacity(frames);
        out.push(self.render());
        for _ in 1..frames {
            #[expect(
                clippy::cast_precision_loss,
                reason = "interval_ms is clamped to MAX_FILM_INTERVAL_MS, far under f64's exact-integer range"
            )]
            self.pump(interval_ms as f64);
            out.push(self.render());
        }
        out
    }
}
