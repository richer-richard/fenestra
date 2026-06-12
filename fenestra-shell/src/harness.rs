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
//! structural tests stay fast.

use std::sync::{Arc, Mutex, PoisonError};

use fenestra_core::{
    AccessNode, App, Element, Frame, FrameState, InputEvent, KeyInput, Proxy, Query, Theme,
    build_frame, dispatch,
};
use image::RgbaImage;

use crate::element_render::with_fonts;
use crate::with_headless;

/// A headless app under test. See the module docs for the model.
pub struct Harness<A: App> {
    app: A,
    theme: Theme,
    state: FrameState,
    logical: (f32, f32),
    size: (u32, u32),
    /// Deterministic clock in seconds, advanced only by [`Self::pump`].
    clock: f64,
    /// Messages emitted by handlers since the last [`Self::take_messages`].
    msgs: Vec<A::Msg>,
    pending: Arc<Mutex<Vec<A::Msg>>>,
    view: Element<A::Msg>,
    frame: Frame,
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
        let mut state = FrameState::new();
        state.reduced_motion = true;
        #[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
        let logical = (size.0 as f32, size.1 as f32);
        let view = app.view();
        let frame = with_fonts(|fonts| build_frame(&view, &theme, fonts, &mut state, logical, 1.0));
        Self {
            app,
            theme,
            state,
            logical,
            size,
            clock: 0.0,
            msgs: Vec::new(),
            pending,
            view,
            frame,
        }
    }

    fn drain(app: &mut A, pending: &Mutex<Vec<A::Msg>>) {
        let msgs = std::mem::take(&mut *pending.lock().unwrap_or_else(PoisonError::into_inner));
        for msg in msgs {
            app.update(msg);
        }
    }

    /// Rebuilds the view and frame from current app state (proxied
    /// messages drain first). Runs automatically after every input;
    /// call it yourself only after mutating via [`Self::app_mut`].
    pub fn rebuild(&mut self) {
        Self::drain(&mut self.app, &self.pending);
        self.view = self.app.view();
        self.state.tick(self.clock);
        self.frame = with_fonts(|fonts| {
            build_frame(
                &self.view,
                &self.theme,
                fonts,
                &mut self.state,
                self.logical,
                1.0,
            )
        });
    }

    /// Dispatches one raw input event against the current frame, logs
    /// and applies the emitted messages, and rebuilds.
    pub fn input(&mut self, event: InputEvent) {
        let result =
            with_fonts(|fonts| dispatch(&self.view, &self.frame, &mut self.state, fonts, event));
        for msg in result.msgs {
            self.msgs.push(msg.clone());
            self.app.update(msg);
        }
        self.rebuild();
    }

    fn center(&self, q: &Query) -> (f32, f32) {
        let node = self.frame.get(q);
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
        let id = self.frame.get(q).id;
        self.state.set_focus(Some(id));
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
        self.input(InputEvent::Wheel { dy });
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
        self.frame.get(q)
    }

    /// The single matching node, or `None`. Use to assert absence.
    ///
    /// # Panics
    /// If the query matches several nodes.
    pub fn query(&self, q: &Query) -> Option<AccessNode> {
        self.frame.query(q)
    }

    /// Every matching node in tree order.
    pub fn get_all(&self, q: &Query) -> Vec<AccessNode> {
        self.frame.get_all(q)
    }

    /// Messages emitted by handlers since the last call (the Elm-level
    /// assertion: *what the UI said*, independent of state effects).
    /// Proxied and [`Self::update`] messages are inputs, not logged.
    pub fn take_messages(&mut self) -> Vec<A::Msg> {
        std::mem::take(&mut self.msgs)
    }

    /// The current frame, for direct queries and `access_yaml()`.
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// The app under test.
    pub fn app(&self) -> &A {
        &self.app
    }

    /// Mutable access to the app; call [`Self::rebuild`] afterwards.
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    /// Renders the current frame to pixels. Mid-test captures are fine —
    /// the frame is not consumed.
    ///
    /// # Panics
    /// If rendering fails.
    pub fn render(&mut self) -> RgbaImage {
        let scene = with_fonts(|fonts| self.frame.paint(fonts, &mut self.state));
        with_headless(|h| h.render(&scene, self.size.0, self.size.1, self.theme.bg))
            .expect("headless renderer unavailable")
            .expect("headless render failed")
    }
}
