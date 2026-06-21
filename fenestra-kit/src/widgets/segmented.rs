//! A segmented control: a compact, single-select view/option switcher.
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
//!     segmented(0, ["List", "Board", "Calendar"], Msg::View);
//! ```

use fenestra_core::{
    Cursor, Element, SP3, Semantics, ShadowToken, Theme, Transition, Weight, div, row, text,
};

use super::ControlSize;

/// Inner padding of the track around the segments, logical px. The active
/// thumb's radius is the track radius minus this, so the corners stay
/// concentric.
const TRACK_PAD: f32 = 3.0;

/// A segmented control — a compact, single-select switcher for a few mutually
/// exclusive options (list / board / calendar, day / week / month). `active`
/// is the selected index; choosing a segment emits `on_select(index)`. Unlike
/// [`tabs`](crate::tabs) (which navigate), the segments sit in a contained
/// track with a raised "thumb" behind the active one — the denser, more
/// "app-grade" affordance for switching a view.
///
/// Elm-pure: the control is a pure function of `active`; the host owns the
/// state and echoes the new index back through `on_select`. The thumb
/// cross-fades between segments (the same deliberate choice [`tabs`] makes: a
/// true position slide needs measured-position animation the per-element
/// transition engine does not do).
pub fn segmented<Msg>(
    active: usize,
    labels: impl IntoIterator<Item = impl Into<String>>,
    on_select: impl Fn(usize) -> Msg,
) -> Element<Msg> {
    let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
    let m = ControlSize::Md.metrics();
    let seg_h = (m.height - 2.0 * TRACK_PAD).max(0.0);

    let segments = labels.into_iter().enumerate().map(move |(i, label)| {
        let is_active = i == active;
        let mut seg = div()
            .h(seg_h)
            .px(SP3)
            .items_center()
            .justify_center()
            .shrink0()
            .focusable(true)
            .cursor(Cursor::Pointer)
            .on_click(on_select(i))
            .semantics(Semantics::Tab {
                selected: is_active,
            })
            .label(label.clone())
            .transition(Transition::colors())
            .themed(move |t: &Theme, s| {
                let s = s.rounded((t.radius.md - TRACK_PAD).max(0.0));
                if is_active {
                    s.bg(t.surface_raised)
                } else {
                    // Transparent neutral base so the thumb fill fades in from
                    // nothing rather than snapping.
                    s.bg(t.element.with_alpha(0.0))
                }
            })
            .children([text(label)
                .size(m.font)
                .weight(Weight::Medium)
                .transition(Transition::colors())
                .themed(move |t: &Theme, s| {
                    s.color(if is_active { t.text } else { t.text_muted })
                })]);
        if is_active {
            seg = seg.shadow(ShadowToken::Sm);
        }
        seg
    });

    row()
        .p(TRACK_PAD)
        .shrink0()
        .themed(|t: &Theme, s| s.rounded(t.radius.md).bg(t.element))
        .children(segments)
}
