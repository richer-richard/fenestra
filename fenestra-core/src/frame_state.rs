//! All retained UI state lives here, keyed by stable `WidgetId`s: scroll
//! offsets today; hover/focus/caret/animation clocks in M4+. Everything else
//! in the pipeline is a pure function of `(tree, theme, size, scale)`.

use std::collections::HashMap;

use crate::anim::Anim;
use crate::clipboard::{Clipboard, MemoryClipboard};
use crate::id::WidgetId;
use crate::input::EditorState;

/// How long the scrollbar stays fully visible after the last scroll.
const SCROLLBAR_HOLD_SECS: f64 = 0.8;
/// How long the scrollbar takes to fade out after the hold.
const SCROLLBAR_FADE_SECS: f64 = 0.3;

#[derive(Debug, Clone, Copy, Default)]
struct Scroll {
    offset_y: f32,
    last_change: f64,
}

/// Retained state for one UI surface (window or headless session).
pub struct FrameState {
    /// Monotonic time in seconds, advanced by the runner via [`Self::tick`].
    now: f64,
    scroll: HashMap<WidgetId, Scroll>,
    /// Snaps every animation to its final value. Headless rendering sets it.
    pub reduced_motion: bool,
    /// Elements currently hovered (with hover styling or handlers), with
    /// the clock time the hover began (tooltips key off it).
    pub(crate) hovered: HashMap<WidgetId, f64>,
    /// The pressed element; it captures the pointer until release.
    pub(crate) active: Option<WidgetId>,
    /// The focused element.
    pub(crate) focus: Option<WidgetId>,
    /// Whether focus arrived via keyboard (paints the focus ring).
    pub(crate) focus_visible: bool,
    /// Last pointer position in logical coordinates.
    pub(crate) pointer: Option<(f32, f32)>,
    /// In-flight style transitions per widget.
    pub(crate) anims: HashMap<WidgetId, Anim>,
    /// Text editor state per input widget.
    pub(crate) editors: HashMap<WidgetId, EditorState>,
    /// Open overlay ids, bottom to top.
    pub(crate) overlays: Vec<WidgetId>,
    /// First-build time of each open overlay (drives enter animations).
    pub(crate) overlay_opened: HashMap<WidgetId, f64>,
    /// The clipboard; the shell injects the OS clipboard, headless keeps
    /// the in-memory default.
    pub(crate) clipboard: Box<dyn Clipboard>,
    /// Frame stamp for animation garbage collection.
    pub(crate) frame_no: u64,
}

impl Default for FrameState {
    fn default() -> Self {
        Self {
            now: 0.0,
            scroll: HashMap::new(),
            reduced_motion: false,
            hovered: HashMap::new(),
            active: None,
            focus: None,
            focus_visible: false,
            pointer: None,
            anims: HashMap::new(),
            editors: HashMap::new(),
            overlays: Vec::new(),
            overlay_opened: HashMap::new(),
            clipboard: Box::new(MemoryClipboard::default()),
            frame_no: 0,
        }
    }
}

impl FrameState {
    /// Creates empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Advances the clock (seconds since an arbitrary start).
    pub fn tick(&mut self, now_seconds: f64) {
        self.now = now_seconds;
    }

    /// The current clock value.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Whether the id is hovered.
    pub fn is_hovered(&self, id: WidgetId) -> bool {
        self.hovered.contains_key(&id)
    }

    /// How long the id has been hovered, in seconds.
    pub(crate) fn hovered_for(&self, id: WidgetId) -> Option<f64> {
        self.hovered.get(&id).map(|t| self.now - t)
    }

    /// Whether the overlay id is open.
    pub fn overlay_open(&self, id: WidgetId) -> bool {
        self.overlays.contains(&id)
    }

    /// Opens an overlay (pushes onto the stack).
    pub(crate) fn open_overlay(&mut self, id: WidgetId) {
        if !self.overlays.contains(&id) {
            self.overlays.push(id);
            self.overlay_opened.insert(id, self.now);
        }
    }

    /// Closes an overlay.
    pub(crate) fn close_overlay(&mut self, id: WidgetId) {
        self.overlays.retain(|o| *o != id);
        self.overlay_opened.remove(&id);
    }

    /// Whether the id is pressed.
    pub fn is_active(&self, id: WidgetId) -> bool {
        self.active == Some(id)
    }

    /// The focused widget, if any.
    pub fn focused(&self) -> Option<WidgetId> {
        self.focus
    }

    /// Moves focus programmatically (marks it keyboard-visible).
    pub fn set_focus(&mut self, id: Option<WidgetId>) {
        self.focus = id;
        self.focus_visible = id.is_some();
    }

    /// Replaces the clipboard implementation (the shell injects arboard).
    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = clipboard;
    }

    /// Adds to a scrollable's offset. The offset is clamped to the content
    /// range on the next frame build, when content size is known.
    pub fn scroll_by(&mut self, id: WidgetId, dy: f32) {
        let entry = self.scroll.entry(id).or_default();
        entry.offset_y += dy;
        entry.last_change = self.now;
    }

    /// The persisted scroll offset for an id (0 when never scrolled).
    pub fn scroll_offset(&self, id: WidgetId) -> f32 {
        self.scroll.get(&id).map_or(0.0, |s| s.offset_y)
    }

    /// Clamps a stored offset to `0..=max`, returning the clamped value.
    /// Called during frame builds once the content height is known.
    pub(crate) fn clamp_scroll(&mut self, id: WidgetId, max: f32) -> f32 {
        match self.scroll.get_mut(&id) {
            Some(s) => {
                s.offset_y = s.offset_y.clamp(0.0, max.max(0.0));
                s.offset_y
            }
            None => 0.0,
        }
    }

    /// Scrollbar opacity for an id: 1.0 while scrolling (held briefly), then
    /// fading to 0. With `reduced_motion` the fade is a step function.
    pub(crate) fn scrollbar_alpha(&self, id: WidgetId) -> f32 {
        let Some(s) = self.scroll.get(&id) else {
            return 0.0;
        };
        let age = self.now - s.last_change;
        if age <= SCROLLBAR_HOLD_SECS {
            1.0
        } else if self.reduced_motion {
            0.0
        } else {
            let t = (age - SCROLLBAR_HOLD_SECS) / SCROLLBAR_FADE_SECS;
            #[expect(clippy::cast_possible_truncation, reason = "alpha fits in f32")]
            {
                (1.0 - t).clamp(0.0, 1.0) as f32
            }
        }
    }

    /// `true` while a scrollbar is mid-fade (the runner should keep
    /// scheduling frames).
    pub(crate) fn scrollbar_animating(&self, id: WidgetId) -> bool {
        if self.reduced_motion {
            return false;
        }
        self.scroll.get(&id).is_some_and(|s| {
            let age = self.now - s.last_change;
            age > 0.0 && age <= SCROLLBAR_HOLD_SECS + SCROLLBAR_FADE_SECS
        })
    }
}
