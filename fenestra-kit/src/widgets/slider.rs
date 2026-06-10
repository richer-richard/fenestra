//! Slider: a 4px track with accent fill and a draggable 16px thumb that
//! grows to 18px on hover. Arrow keys step the value while focused.
//!
//! ```
//! use fenestra_kit::slider;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Volume(f32),
//! }
//!
//! let el: fenestra_core::Element<Msg> = slider(0.4).on_change(Msg::Volume).into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, Length, R_FULL, ShadowToken, Theme, Transition, div, row,
};

/// A slider under construction; converts into an [`Element`].
pub struct Slider<Msg> {
    value: f32,
    width: f32,
    step: f32,
    disabled: bool,
    on_change: Option<std::rc::Rc<dyn Fn(f32) -> Msg>>,
    key: Option<String>,
}

/// A slider over the 0.0..=1.0 range showing `value`.
pub fn slider<Msg>(value: f32) -> Slider<Msg> {
    Slider {
        value: value.clamp(0.0, 1.0),
        width: 200.0,
        step: 0.05,
        disabled: false,
        on_change: None,
        key: None,
    }
}

const TRACK_H: f32 = 4.0;
const THUMB: f32 = 16.0;
const HEIGHT: f32 = 20.0;

impl<Msg> Slider<Msg> {
    /// Sets the slider width in logical px (200 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(THUMB * 2.0);
        self
    }

    /// Sets the keyboard step (0.05 by default).
    pub fn step(mut self, step: f32) -> Self {
        self.step = step.clamp(0.001, 1.0);
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps the new value to a message on drag, click, or arrow keys.
    pub fn on_change(mut self, f: impl Fn(f32) -> Msg + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: 'static> From<Slider<Msg>> for Element<Msg> {
    fn from(sl: Slider<Msg>) -> Self {
        let value = sl.value;
        let width = sl.width;
        let step = sl.step;

        let fill = div()
            .h_full()
            .rounded(R_FULL)
            .w(Length::Pct(value * 100.0))
            .themed(|t: &Theme, s| s.bg(t.accent));

        let track = div()
            .w_full()
            .h(TRACK_H)
            .rounded(R_FULL)
            .children([fill])
            .themed(|t: &Theme, s| s.bg(t.neutrals.step(5)));

        let thumb_left = value * (width - THUMB);
        let thumb = div()
            .absolute()
            .top((HEIGHT - THUMB) / 2.0)
            .left(thumb_left)
            .w(THUMB)
            .h(THUMB)
            .rounded_full()
            .shadow(ShadowToken::Sm)
            .cursor(Cursor::Pointer)
            .disabled(sl.disabled)
            .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border))
            .hover_themed(move |t, s| {
                // Grow 16 -> 18 around the same center (offset both axes).
                s.w(THUMB + 2.0)
                    .h(THUMB + 2.0)
                    .top((HEIGHT - THUMB) / 2.0 - 1.0)
                    .left(thumb_left - 1.0)
                    .border(1.0, t.border_strong)
            })
            .transition(Transition::colors().lengths(true).offsets(true));

        let mut el = row()
            .items_center()
            .w(width)
            .h(HEIGHT)
            .shrink0()
            .children([track, thumb])
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(sl.disabled);

        if let Some(f) = sl.on_change {
            let map = {
                let f = f.clone();
                move |v: f32| {
                    let stepped = (v / step).round() * step;
                    f(stepped.clamp(0.0, 1.0))
                }
            };
            let drag_map = map.clone();
            el = el.on_drag(move |fx, _fy| {
                // Position the thumb center under the pointer.
                let px = fx * width;
                let v = (px - THUMB / 2.0) / (width - THUMB);
                Some(drag_map(v.clamp(0.0, 1.0)))
            });
            let key_map = map;
            el = el.on_key(move |k| match k.key {
                Key::ArrowLeft | Key::ArrowDown => Some(key_map((value - step).max(0.0))),
                Key::ArrowRight | Key::ArrowUp => Some(key_map((value + step).min(1.0))),
                Key::Home => Some(key_map(0.0)),
                Key::End => Some(key_map(1.0)),
                _ => None,
            });
        }
        if sl.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &sl.key {
            el = el.id(key);
        }
        el
    }
}
