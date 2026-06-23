//! All retained UI state lives here, keyed by stable `WidgetId`s: scroll
//! offsets today; hover/focus/caret/animation clocks in M4+. Everything else
//! in the pipeline is a pure function of `(tree, theme, size, scale)`.

use std::collections::HashMap;

use crate::anim::Anim;
use crate::clipboard::{Clipboard, MemoryClipboard};
use crate::element::ExitAnim;
use crate::ghost::GhostNode;
use crate::id::WidgetId;
use crate::input::EditorState;

/// One in-flight exit animation: the paint-only snapshot left behind by a
/// departed element, its targets and timing, the clock time it began, and
/// whether it has finished playing (a settled record is garbage-collected at
/// the top of the next frame build).
pub(crate) struct ExitRecord {
    /// The frozen subtree to paint.
    pub(crate) ghost: GhostNode,
    /// Exit targets and timing.
    pub(crate) exit: ExitAnim,
    /// Clock time the exit started.
    pub(crate) t0: f64,
    /// Whether the animation has completed (paint advances this; the next
    /// build drops it). Seeded `true` under reduced motion so removal is
    /// instant and headless renders are unchanged.
    pub(crate) settled: bool,
}

/// How long the scrollbar stays fully visible after the last scroll.
const SCROLLBAR_HOLD_SECS: f64 = 0.8;
/// How long the scrollbar takes to fade out after the hold.
const SCROLLBAR_FADE_SECS: f64 = 0.3;

#[derive(Debug, Clone, Copy, Default)]
struct Scroll {
    offset_y: f32,
    offset_x: f32,
    last_change: f64,
    /// Frame stamp for garbage collection, like `Anim::seen`.
    seen: u64,
    /// Sat at the bottom edge after the last clamp (stick-to-bottom).
    at_bottom: bool,
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
    /// Where and when the active press began (x, y, seconds) — for swipe
    /// recognition on release.
    pub(crate) press_origin: Option<(f32, f32, f64)>,
    /// In-flight style transitions per widget.
    pub(crate) anims: HashMap<WidgetId, Anim>,
    /// Every node's absolute rect from the frame just built, keyed by id —
    /// the previous-frame measurements FLIP layout animation slides from (and
    /// shared with the constraints-aware-layout work). Replaced wholesale each
    /// frame after realize.
    pub(crate) prev_rects: HashMap<WidgetId, kurbo::Rect>,
    /// Exit-tagged live nodes from the frame just built (id -> snapshot +
    /// targets). Next frame, any id here but absent from the new tree has
    /// departed and starts an exit animation.
    pub(crate) exit_cache: HashMap<WidgetId, (GhostNode, ExitAnim)>,
    /// Exit animations currently playing (a departed element's lingering
    /// ghost). Painted on top of the frame; GC'd once settled.
    pub(crate) exiting: HashMap<WidgetId, ExitRecord>,
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
    /// Last completed click (element, clock time), for double-click.
    pub(crate) last_click: Option<(WidgetId, f64)>,
    /// Press-time click counting on inputs (word/line selection):
    /// target, time, and how many presses in the chain (1..=3).
    pub(crate) last_press: Option<(WidgetId, f64, u8)>,
    /// Modifier keys as last reported by [`InputEvent::Modifiers`]:
    /// (shift, ctrl, alt, meta).
    pub(crate) mods: (bool, bool, bool, bool),
    /// Active static-text selection (one at a time, like a browser):
    /// element, parley selection, and whether a drag is extending it.
    pub(crate) static_sel: Option<(WidgetId, parley::Selection, bool)>,
    /// Focused-element type-ahead: target, buffer, last keystroke time.
    pub(crate) type_ahead: Option<(WidgetId, String, f64)>,
    /// Variable-height virtual lists: measured row heights per
    /// container, estimates for the rest (self-correcting offsets).
    pub(crate) virtual_heights: HashMap<WidgetId, HeightIndex>,
    /// Materialized windows this frame (container -> row range), so the
    /// post-layout pass knows which child maps to which row index.
    pub(crate) virtual_windows: HashMap<WidgetId, std::ops::Range<usize>>,
    /// The autofocus element and the frame it was last seen, so focus
    /// moves only when it newly appears.
    pub(crate) autofocus_last: Option<(WidgetId, u64)>,
    /// In-flight internal drag payload (from `.drag_source`).
    pub(crate) dragging: Option<String>,
    /// Caret rect of the focused editor from the last paint, in logical
    /// coordinates — the runner positions the IME popup with it.
    pub(crate) ime_caret: Option<kurbo::Rect>,
    /// Pointer position captured when a pointer-placed overlay opened, so
    /// context menus stay pinned while the mouse moves.
    pub(crate) pointer_pins: std::collections::HashMap<WidgetId, (f32, f32)>,
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
            press_origin: None,
            anims: HashMap::new(),
            prev_rects: HashMap::new(),
            exit_cache: HashMap::new(),
            exiting: HashMap::new(),
            editors: HashMap::new(),
            overlays: Vec::new(),
            overlay_opened: HashMap::new(),
            clipboard: Box::new(MemoryClipboard::default()),
            frame_no: 0,
            last_click: None,
            last_press: None,
            mods: (false, false, false, false),
            static_sel: None,
            type_ahead: None,
            virtual_heights: HashMap::new(),
            virtual_windows: HashMap::new(),
            autofocus_last: None,
            dragging: None,
            ime_caret: None,
            pointer_pins: std::collections::HashMap::new(),
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
        self.pointer_pins.remove(&id);
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
        // Recomputed at the next clamp; a manual move unpins the bottom.
        entry.at_bottom = false;
    }

    /// The persisted scroll offset for an id (0 when never scrolled).
    pub fn scroll_offset(&self, id: WidgetId) -> f32 {
        self.scroll.get(&id).map_or(0.0, |s| s.offset_y)
    }

    /// The focused editor's caret rect from the last paint (logical
    /// coordinates); the windowed runner anchors the IME candidate window
    /// to it.
    pub fn ime_caret(&self) -> Option<kurbo::Rect> {
        self.ime_caret
    }

    /// Sets a scrollable's offset absolutely (clamped to the content range
    /// on the next frame build; `f32::MAX` means "the bottom").
    pub fn scroll_to(&mut self, id: WidgetId, offset_y: f32) {
        let now = self.now;
        let entry = self.scroll.entry(id).or_default();
        entry.offset_y = offset_y.max(0.0);
        entry.last_change = now;
        // Recomputed at the next clamp; a manual move unpins the bottom.
        entry.at_bottom = false;
    }

    /// Adds to a scrollable's horizontal offset (clamped on the next build).
    pub fn scroll_by_x(&mut self, id: WidgetId, dx: f32) {
        let entry = self.scroll.entry(id).or_default();
        entry.offset_x += dx;
        entry.last_change = self.now;
    }

    /// The persisted horizontal scroll offset for an id (0 when never scrolled).
    pub fn scroll_offset_x(&self, id: WidgetId) -> f32 {
        self.scroll.get(&id).map_or(0.0, |s| s.offset_x)
    }

    /// Sets a scrollable's horizontal offset absolutely (clamped on the next build).
    pub fn scroll_to_x(&mut self, id: WidgetId, offset_x: f32) {
        let now = self.now;
        let entry = self.scroll.entry(id).or_default();
        entry.offset_x = offset_x.max(0.0);
        entry.last_change = now;
    }

    /// Clamps both axes of a stored offset to `0..=max`, returning `(y, x)`.
    /// Called during frame builds once content size is known; stamps the entry
    /// alive for [`Self::gc_scroll`] and applies the stick-to-bottom rule
    /// (pinned while at the vertical bottom edge). The horizontal axis has no
    /// stick rule.
    pub(crate) fn clamp_scroll_2d(
        &mut self,
        id: WidgetId,
        max_y: f32,
        max_x: f32,
        stick_bottom: bool,
    ) -> (f32, f32) {
        let frame_no = self.frame_no;
        let max_y = max_y.max(0.0);
        let max_x = max_x.max(0.0);
        match self.scroll.get_mut(&id) {
            Some(s) => {
                if stick_bottom && s.at_bottom {
                    s.offset_y = max_y;
                }
                s.offset_y = s.offset_y.clamp(0.0, max_y);
                s.offset_x = s.offset_x.clamp(0.0, max_x);
                s.at_bottom = s.offset_y >= max_y - 1.0;
                s.seen = frame_no;
                (s.offset_y, s.offset_x)
            }
            None if stick_bottom => {
                // Sticky containers start at the bottom, scrollbar quiet.
                self.scroll.insert(
                    id,
                    Scroll {
                        offset_y: max_y,
                        offset_x: 0.0,
                        last_change: -10.0,
                        seen: frame_no,
                        at_bottom: true,
                    },
                );
                (max_y, 0.0)
            }
            None => (0.0, 0.0),
        }
    }

    /// Drops scroll entries whose container was not in the frame just built,
    /// so dynamically keyed scrollables cannot grow the map without bound.
    pub(crate) fn gc_scroll(&mut self, frame_no: u64) {
        self.scroll.retain(|_, s| s.seen == frame_no);
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

    // ---- motion introspection (test support; hidden from the public docs) ----
    //
    // Integration tests live in their own crate and link the library built
    // without `cfg(test)`, so a `#[cfg(test)]` accessor would be invisible to
    // them; these are plain `pub` + `#[doc(hidden)]` instead — the same shape
    // the existing `scroll_offset` test hooks use.

    /// Whether a retained style/FLIP animation is currently held for `id`.
    #[doc(hidden)]
    pub fn has_anim(&self, id: WidgetId) -> bool {
        self.anims.contains_key(&id)
    }

    /// Whether an exit animation is tracked for `id` (in flight, or settled
    /// but not yet garbage-collected).
    #[doc(hidden)]
    pub fn has_exiting(&self, id: WidgetId) -> bool {
        self.exiting.contains_key(&id)
    }

    /// Whether the exit animation for `id` has settled, or `None` when no exit
    /// is tracked for it.
    #[doc(hidden)]
    pub fn exiting_settled(&self, id: WidgetId) -> Option<bool> {
        self.exiting.get(&id).map(|r| r.settled)
    }

    /// The previous frame's measured rect for `id` (what FLIP slides from),
    /// if one was recorded.
    #[doc(hidden)]
    pub fn prev_rect(&self, id: WidgetId) -> Option<kurbo::Rect> {
        self.prev_rects.get(&id).copied()
    }

    /// The exit ghost's frozen paint-time translate for `id` (what the leaving
    /// element last painted with, including any in-flight FLIP slide), or `None`
    /// when no exit is tracked for it.
    #[doc(hidden)]
    pub fn exiting_ghost_translate(&self, id: WidgetId) -> Option<(f32, f32)> {
        self.exiting.get(&id).map(|r| r.ghost.style.translate)
    }
}

/// Row-height bookkeeping for one variable-height virtual list:
/// measured heights where rows have materialized, the estimate
/// elsewhere, and a prefix-sum index for offset math.
#[derive(Debug)]
pub(crate) struct HeightIndex {
    estimate: f32,
    heights: Vec<f32>,
    /// prefix[i] = offset of row i; prefix[count] = total height.
    prefix: Vec<f32>,
    dirty: bool,
}

impl HeightIndex {
    pub(crate) fn ensure(&mut self, count: usize, estimate: f32) {
        let estimate = if estimate.is_finite() && estimate > 0.0 {
            estimate
        } else {
            1.0
        };
        if self.heights.len() != count || (self.estimate - estimate).abs() > f32::EPSILON {
            self.estimate = estimate;
            self.heights = vec![estimate; count];
            self.dirty = true;
        }
        if self.dirty {
            self.prefix.clear();
            self.prefix.reserve(count + 1);
            let mut acc = 0.0;
            self.prefix.push(0.0);
            for h in &self.heights {
                acc += h;
                self.prefix.push(acc);
            }
            self.dirty = false;
        }
    }

    pub(crate) fn new_with(count: usize, estimate: f32) -> Self {
        let mut index = Self {
            estimate: 1.0,
            heights: Vec::new(),
            prefix: Vec::new(),
            dirty: true,
        };
        index.ensure(count, estimate);
        index
    }

    /// Offset of a row's top edge.
    pub(crate) fn offset_of(&self, i: usize) -> f32 {
        self.prefix.get(i).copied().unwrap_or_default()
    }

    /// Total content height.
    pub(crate) fn total(&self) -> f32 {
        self.prefix.last().copied().unwrap_or_default()
    }

    /// The row containing `offset` (binary search over the prefix).
    pub(crate) fn index_at(&self, offset: f32) -> usize {
        if self.heights.is_empty() {
            return 0;
        }
        let offset = offset.max(0.0);
        match self
            .prefix
            .binary_search_by(|p| p.partial_cmp(&offset).unwrap_or(std::cmp::Ordering::Less))
        {
            Ok(i) => i.min(self.heights.len().saturating_sub(1)),
            Err(i) => i
                .saturating_sub(1)
                .min(self.heights.len().saturating_sub(1)),
        }
    }

    /// Records a measured row height; offsets correct on the next
    /// frame's `ensure`.
    pub(crate) fn record(&mut self, i: usize, height: f32) {
        if height.is_finite()
            && height > 0.0
            && let Some(slot) = self.heights.get_mut(i)
            && (*slot - height).abs() > 0.25
        {
            *slot = height;
            self.dirty = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::HeightIndex;

    #[test]
    fn prefix_sums_follow_the_estimate() {
        let idx = HeightIndex::new_with(5, 30.0);
        assert_eq!(idx.offset_of(0), 0.0);
        assert_eq!(idx.offset_of(3), 90.0);
        assert_eq!(idx.total(), 150.0);
        // Boundaries: a row's top belongs to that row.
        assert_eq!(idx.index_at(0.0), 0);
        assert_eq!(idx.index_at(29.9), 0);
        assert_eq!(idx.index_at(30.0), 1);
        // Out-of-range clamps instead of panicking.
        assert_eq!(idx.index_at(-5.0), 0);
        assert_eq!(idx.index_at(1.0e9), 4);
        assert_eq!(idx.offset_of(99), 0.0, "missing rows read as origin");
    }

    #[test]
    fn recorded_heights_correct_downstream_offsets() {
        let mut idx = HeightIndex::new_with(5, 30.0);
        idx.record(1, 64.0);
        idx.ensure(5, 30.0); // the next frame rebuilds the prefix
        assert_eq!(idx.offset_of(1), 30.0, "upstream rows are untouched");
        assert_eq!(idx.offset_of(2), 94.0, "downstream rows shift");
        assert_eq!(idx.total(), 184.0);
        assert_eq!(idx.index_at(93.0), 1, "the taller row owns its span");
        assert_eq!(idx.index_at(94.0), 2);
    }

    #[test]
    fn hostile_measurements_and_jitter_are_ignored() {
        let mut idx = HeightIndex::new_with(3, 30.0);
        idx.record(0, f32::NAN);
        idx.record(0, f32::INFINITY);
        idx.record(0, 0.0);
        idx.record(0, -12.0);
        idx.record(1, 30.1); // sub-quarter-pixel jitter must not thrash
        idx.record(99, 50.0); // out of range
        idx.ensure(3, 30.0);
        assert_eq!(idx.total(), 90.0, "nothing above changed the index");
    }

    #[test]
    fn count_and_estimate_changes_reset_measurements() {
        let mut idx = HeightIndex::new_with(3, 30.0);
        idx.record(0, 64.0);
        idx.ensure(3, 30.0);
        assert_eq!(idx.total(), 124.0);

        idx.ensure(4, 30.0); // row count changed: re-seed everything
        assert_eq!(idx.total(), 120.0);

        idx.record(0, 64.0);
        idx.ensure(4, 20.0); // estimate changed: re-seed too
        assert_eq!(idx.total(), 80.0);

        idx.ensure(4, f32::NAN); // hostile estimate falls back to 1.0
        assert_eq!(idx.total(), 4.0);
    }

    #[test]
    fn empty_index_is_inert() {
        let idx = HeightIndex::new_with(0, 30.0);
        assert_eq!(idx.total(), 0.0);
        assert_eq!(idx.index_at(10.0), 0);
        assert_eq!(idx.offset_of(0), 0.0);
    }
}
