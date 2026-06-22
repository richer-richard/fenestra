//! A segmented control: a compact, single-select view/option switcher with a
//! thumb that *slides* between segments on a spring.
//!
//! ```
//! use fenestra_kit::segmented;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     View(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> =
//!     segmented(0, ["List", "Board", "Calendar"], Msg::View).into();
//! ```

use fenestra_core::{
    Cursor, Element, Key, Semantics, ShadowToken, Theme, Transition, Weight, div, row, text,
};

use super::ControlSize;

/// Inner padding of the track around the segments, logical px. The thumb's
/// corner radius is the track radius minus this, so the corners stay concentric.
const TRACK_PAD: f32 = 3.0;

/// A segmented control under construction; converts into an [`Element`].
pub struct Segmented<Msg> {
    active: usize,
    labels: Vec<String>,
    on_select: Box<dyn Fn(usize) -> Msg>,
    size: ControlSize,
    width: Option<f32>,
    disabled: bool,
}

/// A segmented control — a compact, single-select switcher for a few mutually
/// exclusive options (list / board / calendar, day / week / month). `active`
/// is the selected index; choosing a segment emits `on_select(index)`. Unlike
/// [`tabs`](crate::tabs) (which navigate), the segments sit in a contained
/// track with a raised thumb that **slides** to the active segment on a spring
/// — the denser, more "app-grade" affordance for switching a view.
///
/// Segments are equal width (sized to the longest label by default; override
/// the whole control's width with [`Segmented::width`]). Elm-pure: the control
/// is a pure function of `active`; the host owns the state and echoes the new
/// index back through `on_select`.
pub fn segmented<Msg>(
    active: usize,
    labels: impl IntoIterator<Item = impl Into<String>>,
    on_select: impl Fn(usize) -> Msg + 'static,
) -> Segmented<Msg> {
    Segmented {
        active,
        labels: labels.into_iter().map(Into::into).collect(),
        on_select: Box::new(on_select),
        size: ControlSize::Md,
        width: None,
        disabled: false,
    }
}

impl<Msg> Segmented<Msg> {
    /// Sets the control height via [`ControlSize`] (Sm 32 / Md 36 / Lg 40 px).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Pins the total control width (logical px); segments split it equally.
    /// Without this, the control sizes every segment to the longest label.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Disables the whole control (dimmed, non-interactive).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<Msg: 'static> From<Segmented<Msg>> for Element<Msg> {
    fn from(s: Segmented<Msg>) -> Self {
        let n = s.labels.len().max(1);
        let m = s.size.metrics();
        let pad = TRACK_PAD;

        // Equal segment width: the caller's width split n ways, or an estimate
        // from the longest label (≈ the Sm/Base advance) so short labels don't
        // sprawl and long ones aren't clipped.
        let seg_w = match s.width {
            Some(w) => ((w - 2.0 * pad) / n as f32).max(24.0),
            None => {
                let max_chars = s
                    .labels
                    .iter()
                    .map(|l| l.chars().count())
                    .max()
                    .unwrap_or(1);
                let char_px = if matches!(s.size, ControlSize::Lg) {
                    8.5
                } else {
                    7.5
                };
                (max_chars as f32 * char_px + 2.0 * m.pad_x).max(48.0)
            }
        };
        let seg_h = (m.height - 2.0 * pad).max(0.0);
        let total_w = seg_w * n as f32 + 2.0 * pad;
        let active = s.active.min(n.saturating_sub(1));
        // Shared so both the per-segment click handlers and the control-level
        // key handler can echo the chosen index back to the host.
        let on_select: std::rc::Rc<dyn Fn(usize) -> Msg> = s.on_select.into();
        let labels = s.labels;
        let disabled = s.disabled;

        // The sliding thumb: one absolutely-positioned element whose `left`
        // retargets to the active segment, animated on a spatial spring — so it
        // travels rather than cross-fades. A single stable id (child 0 of the
        // track) keeps the transition continuous across rebuilds.
        let thumb = div::<Msg>()
            .absolute()
            .top(pad)
            .left(pad + active as f32 * seg_w)
            .w(seg_w)
            .h(seg_h)
            .themed(move |t: &Theme, st| {
                st.rounded((t.radius.md - pad).max(0.0))
                    .bg(t.surface_raised)
            })
            .shadow(ShadowToken::Sm)
            .transition(Transition::colors().offsets(true).with_spring(380.0, 30.0));

        let seg_select = on_select.clone();
        let segments = row().children((0..n).map(move |i| {
            let is_active = i == active;
            let label = labels[i].clone();
            let mut seg = row()
                .w(seg_w)
                .h(seg_h)
                .items_center()
                .justify_center()
                .shrink0()
                .semantics(Semantics::Tab {
                    selected: is_active,
                })
                .label(label.clone())
                .children([text(label)
                    .size(m.font)
                    .weight(Weight::Medium)
                    .truncate()
                    .transition(Transition::colors())
                    .themed(move |t: &Theme, st| {
                        st.color(if disabled {
                            t.text_disabled
                        } else if is_active {
                            t.text
                        } else {
                            t.text_muted
                        })
                    })]);
            if !disabled {
                // Segments are pointer targets but NOT individual tab stops; the
                // whole control is one tab stop (see the track's key handler).
                // `on_click` auto-focuses, so opt back out.
                seg = seg
                    .cursor(Cursor::Pointer)
                    .on_click(seg_select(i))
                    .focusable(false);
            }
            seg
        }));

        let mut track = div::<Msg>()
            .w(total_w)
            .h(m.height)
            .p(pad)
            .shrink0()
            .themed(|t: &Theme, st| st.rounded(t.radius.md).bg(t.element))
            .children([thumb])
            .children([segments]);
        if disabled {
            track = track.opacity(0.5);
        } else {
            // One tab stop for the whole control; arrows roam the selection
            // (WAI-ARIA tablist keyboard model), Home/End jump to the ends.
            track = track.focusable(true).on_key(move |k| match k.key {
                Key::ArrowRight | Key::ArrowDown => (active + 1 < n).then(|| on_select(active + 1)),
                Key::ArrowLeft | Key::ArrowUp => (active > 0).then(|| on_select(active - 1)),
                Key::Home => Some(on_select(0)),
                Key::End => Some(on_select(n - 1)),
                _ => None,
            });
        }
        track
    }
}
