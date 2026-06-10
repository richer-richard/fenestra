//! All retained UI state lives here, keyed by stable `WidgetId`s: scroll
//! offsets today; hover/focus/caret/animation clocks in M4+. Everything else
//! in the pipeline is a pure function of `(tree, theme, size, scale)`.

use std::collections::HashMap;

use crate::id::WidgetId;

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
#[derive(Debug, Default)]
pub struct FrameState {
    /// Monotonic time in seconds, advanced by the runner via [`Self::tick`].
    now: f64,
    scroll: HashMap<WidgetId, Scroll>,
    /// Snaps every animation to its final value. Headless rendering sets it.
    pub reduced_motion: bool,
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
