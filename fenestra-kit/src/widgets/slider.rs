//! Slider: a 4px track with an accent fill and a draggable thumb. Single-value
//! ([`slider`]) or a two-thumb range ([`range_slider`]), over any numeric
//! domain ([`Slider::range`]). Full keyboard: arrows step, Page keys jump by ten
//! steps, Home/End snap to the ends. Optional step [`marks`](Slider::marks).
//!
//! ```
//! use fenestra_kit::{range_slider, slider};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Volume(f32),
//!     Price(f32, f32),
//! }
//!
//! let v: fenestra_core::Element<Msg> = slider(0.4).on_change(Msg::Volume).into();
//! let p: fenestra_core::Element<Msg> = range_slider(20.0, 80.0)
//!     .range(0.0, 100.0)
//!     .step(5.0)
//!     .on_change(Msg::Price)
//!     .into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, Length, R_FULL, Semantics, Surface, Theme, Transition, div, row,
};

const TRACK_H: f32 = 4.0;
const THUMB: f32 = 16.0;
const HEIGHT: f32 = 20.0;
/// Upper bound on how many step marks are drawn (denser steps render none).
const MAX_MARKS: usize = 50;

/// Snap a domain value to the step grid (anchored at `min`) and clamp to range.
fn snap(v: f32, min: f32, max: f32, step: f32) -> f32 {
    let snapped = min + ((v - min) / step).round() * step;
    snapped.clamp(min.min(max), min.max(max))
}

/// Normalized 0..1 position of `v` within `min..=max`.
fn norm(v: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < f32::EPSILON {
        0.0
    } else {
        ((v - min) / (max - min)).clamp(0.0, 1.0)
    }
}

/// Default step for a domain: a twentieth of the range.
fn default_step(min: f32, max: f32) -> f32 {
    ((max - min) / 20.0).abs().max(1e-9)
}

/// The shared draggable thumb (caller adds focus/keys for the multi-thumb case).
fn thumb_el<Msg>(left: f32, disabled: bool) -> Element<Msg> {
    div()
        .absolute()
        .top((HEIGHT - THUMB) / 2.0)
        .left(left)
        .w(THUMB)
        .h(THUMB)
        .surface(Surface::Thumb)
        .cursor(Cursor::Pointer)
        .disabled(disabled)
        .hover_themed(move |t, s| {
            // Grow 16 -> 18 around the same center (offset both axes).
            s.w(THUMB + 2.0)
                .h(THUMB + 2.0)
                .top((HEIGHT - THUMB) / 2.0 - 1.0)
                .left(left - 1.0)
                .border(1.0, t.border_strong)
        })
        .transition(Transition::colors().lengths(true).offsets(true))
}

/// A 1px step mark centered on the track at normalized position `frac`.
fn mark_el<Msg>(frac: f32, width: f32) -> Element<Msg> {
    div()
        .absolute()
        .top((HEIGHT - 6.0) / 2.0)
        .left(frac * (width - THUMB) + THUMB / 2.0 - 0.5)
        .w(1.0)
        .h(6.0)
        .rounded(R_FULL)
        .themed(|t: &Theme, s| s.bg(t.neutrals.step(6)))
}

/// Step marks across the domain (or none if too dense / unset).
fn marks_layer<Msg>(min: f32, max: f32, step: f32, width: f32, on: bool) -> Vec<Element<Msg>> {
    if !on || step <= 0.0 {
        return Vec::new();
    }
    let count = ((max - min) / step).round() as i64;
    if count <= 0 || count as usize > MAX_MARKS {
        return Vec::new();
    }
    (0..=count)
        .map(|i| {
            let v = min + i as f32 * step;
            mark_el(norm(v, min, max), width)
        })
        .collect()
}

/// A single-value slider under construction; converts into an [`Element`].
pub struct Slider<Msg> {
    value: f32,
    min: f32,
    max: f32,
    width: f32,
    step: Option<f32>,
    marks: bool,
    disabled: bool,
    on_change: Option<std::rc::Rc<dyn Fn(f32) -> Msg>>,
    key: Option<String>,
}

/// A single-value slider showing `value`. The default domain is `0.0..=1.0`;
/// widen it with [`Slider::range`].
pub fn slider<Msg>(value: f32) -> Slider<Msg> {
    Slider {
        value,
        min: 0.0,
        max: 1.0,
        width: 200.0,
        step: None,
        marks: false,
        disabled: false,
        on_change: None,
        key: None,
    }
}

impl<Msg> Slider<Msg> {
    /// Sets the value domain (default `0.0..=1.0`). Values, drags and arrow
    /// steps are all in these units. Ignored unless `max > min`.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if max > min {
            self.min = min;
            self.max = max;
        }
        self
    }

    /// Sets the slider width in logical px (200 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(THUMB * 2.0);
        self
    }

    /// Sets the keyboard/drag step in domain units (default: a twentieth of the
    /// range).
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step.max(1e-9));
        self
    }

    /// Draws a tick at every step position.
    pub fn marks(mut self, on: bool) -> Self {
        self.marks = on;
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps the new value (in domain units) to a message on drag, click, or
    /// arrow/page keys.
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
        let (min, max, width) = (sl.min, sl.max, sl.width);
        let step = sl.step.unwrap_or_else(|| default_step(min, max));
        let value = sl.value.clamp(min, max);
        let frac = norm(value, min, max);

        let fill = div()
            .h_full()
            .rounded(R_FULL)
            .w(Length::Pct(frac * 100.0))
            .themed(|t: &Theme, s| s.bg(t.accent));
        let track = div()
            .w_full()
            .h(TRACK_H)
            .rounded(R_FULL)
            .children([fill])
            .themed(|t: &Theme, s| s.bg(t.neutrals.step(5)));
        let thumb = thumb_el::<Msg>(frac * (width - THUMB), sl.disabled);

        let mut el = row()
            .items_center()
            .w(width)
            .h(HEIGHT)
            .shrink0()
            .child(track)
            .children(marks_layer::<Msg>(min, max, step, width, sl.marks))
            .child(thumb)
            .focusable(true)
            .cursor(Cursor::Pointer)
            .disabled(sl.disabled)
            .semantics(Semantics::Slider { value, min, max });

        if let Some(f) = sl.on_change {
            let drag = f.clone();
            el = el.on_drag(move |fx, _fy| {
                // Position the thumb center under the pointer, then snap.
                let n = ((fx * width - THUMB / 2.0) / (width - THUMB)).clamp(0.0, 1.0);
                Some(drag(snap(min + n * (max - min), min, max, step)))
            });
            let big = step * 10.0;
            el = el.on_key(move |k| match k.key {
                Key::ArrowLeft | Key::ArrowDown => Some(f(snap(value - step, min, max, step))),
                Key::ArrowRight | Key::ArrowUp => Some(f(snap(value + step, min, max, step))),
                Key::PageDown => Some(f(snap(value - big, min, max, step))),
                Key::PageUp => Some(f(snap(value + big, min, max, step))),
                Key::Home => Some(f(min)),
                Key::End => Some(f(max)),
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

/// A two-thumb range slider under construction; converts into an [`Element`].
pub struct RangeSlider<Msg> {
    low: f32,
    high: f32,
    min: f32,
    max: f32,
    width: f32,
    step: Option<f32>,
    marks: bool,
    disabled: bool,
    on_change: Option<std::rc::Rc<dyn Fn(f32, f32) -> Msg>>,
    key: Option<String>,
}

/// A two-thumb range slider selecting `low..=high`. Each thumb is its own tab
/// stop with the full key set; neither thumb can cross the other. The default
/// domain is `0.0..=1.0`; widen it with [`RangeSlider::range`].
pub fn range_slider<Msg>(low: f32, high: f32) -> RangeSlider<Msg> {
    RangeSlider {
        low,
        high,
        min: 0.0,
        max: 1.0,
        width: 200.0,
        step: None,
        marks: false,
        disabled: false,
        on_change: None,
        key: None,
    }
}

impl<Msg> RangeSlider<Msg> {
    /// Sets the value domain (default `0.0..=1.0`). Ignored unless `max > min`.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        if max > min {
            self.min = min;
            self.max = max;
        }
        self
    }

    /// Sets the slider width in logical px (200 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(THUMB * 2.0);
        self
    }

    /// Sets the keyboard/drag step in domain units (default: a twentieth of the
    /// range).
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step.max(1e-9));
        self
    }

    /// Draws a tick at every step position.
    pub fn marks(mut self, on: bool) -> Self {
        self.marks = on;
        self
    }

    /// Disables interaction.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Maps the new `(low, high)` pair to a message on drag or arrow/page keys.
    pub fn on_change(mut self, f: impl Fn(f32, f32) -> Msg + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(f));
        self
    }

    /// Stable identity key.
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

impl<Msg: 'static> From<RangeSlider<Msg>> for Element<Msg> {
    fn from(rs: RangeSlider<Msg>) -> Self {
        let (min, max, width) = (rs.min, rs.max, rs.width);
        let step = rs.step.unwrap_or_else(|| default_step(min, max));
        let mut low = rs.low.clamp(min, max);
        let mut high = rs.high.clamp(min, max);
        if low > high {
            std::mem::swap(&mut low, &mut high);
        }
        let lf = norm(low, min, max);
        let hf = norm(high, min, max);

        let track = div()
            .w_full()
            .h(TRACK_H)
            .rounded(R_FULL)
            .themed(|t: &Theme, s| s.bg(t.neutrals.step(5)));
        let fill = div()
            .absolute()
            .top((HEIGHT - TRACK_H) / 2.0)
            .left(lf * (width - THUMB) + THUMB / 2.0)
            .w((hf - lf) * (width - THUMB))
            .h(TRACK_H)
            .rounded(R_FULL)
            .themed(|t: &Theme, s| s.bg(t.accent));

        let mut low_thumb =
            thumb_el::<Msg>(lf * (width - THUMB), rs.disabled).semantics(Semantics::Slider {
                value: low,
                min,
                max: high,
            });
        let mut high_thumb =
            thumb_el::<Msg>(hf * (width - THUMB), rs.disabled).semantics(Semantics::Slider {
                value: high,
                min: low,
                max,
            });

        if let Some(f) = &rs.on_change
            && !rs.disabled
        {
            let big = step * 10.0;
            let fl = f.clone();
            low_thumb = low_thumb.focusable(true).on_key(move |k| {
                let nl = match k.key {
                    Key::ArrowLeft | Key::ArrowDown => low - step,
                    Key::ArrowRight | Key::ArrowUp => low + step,
                    Key::PageDown => low - big,
                    Key::PageUp => low + big,
                    Key::Home => min,
                    Key::End => high,
                    _ => return None,
                };
                Some(fl(snap(nl, min, high, step), high))
            });
            let fh = f.clone();
            high_thumb = high_thumb.focusable(true).on_key(move |k| {
                let nh = match k.key {
                    Key::ArrowLeft | Key::ArrowDown => high - step,
                    Key::ArrowRight | Key::ArrowUp => high + step,
                    Key::PageDown => high - big,
                    Key::PageUp => high + big,
                    Key::Home => low,
                    Key::End => max,
                    _ => return None,
                };
                Some(fh(low, snap(nh, low, max, step)))
            });
        }

        let mut el = row()
            .items_center()
            .w(width)
            .h(HEIGHT)
            .shrink0()
            .child(track)
            .children(marks_layer::<Msg>(min, max, step, width, rs.marks))
            .child(fill)
            .child(low_thumb)
            .child(high_thumb)
            .disabled(rs.disabled);

        if let Some(f) = &rs.on_change
            && !rs.disabled
        {
            let fd = f.clone();
            el = el.on_drag(move |fx, _fy| {
                let n = ((fx * width - THUMB / 2.0) / (width - THUMB)).clamp(0.0, 1.0);
                let v = min + n * (max - min);
                // Move whichever thumb is nearer the pointer.
                if (v - low).abs() <= (v - high).abs() {
                    Some(fd(snap(v, min, high, step), high))
                } else {
                    Some(fd(low, snap(v, low, max, step)))
                }
            });
        }
        if rs.disabled {
            el = el.opacity(0.5);
        }
        if let Some(key) = &rs.key {
            el = el.id(key);
        }
        el
    }
}
