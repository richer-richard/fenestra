//! Navigation widgets: [`breadcrumbs`] (a trail of links to ancestor pages with
//! a current-page marker and overflow collapse).
//!
//! ```
//! use fenestra_kit::{breadcrumbs, crumb};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Go(usize),
//! }
//!
//! let el: fenestra_core::Element<Msg> = breadcrumbs([
//!     crumb("Home").on_select(Msg::Go(0)),
//!     crumb("Library").on_select(Msg::Go(1)),
//!     crumb("Charts"), // the last crumb is always the current page
//! ])
//! .into();
//! ```

use fenestra_core::{
    Color, Cursor, Element, SP1, Semantics, TextSize, Theme, Transition, Weight, row, text,
};

use crate::icons;

/// One entry in a [`breadcrumbs`] trail.
pub struct Crumb<Msg> {
    label: String,
    icon: Option<Element<Msg>>,
    on_select: Option<Msg>,
    key: Option<String>,
}

/// A breadcrumb entry with the given label. Add `.on_select(msg)` to make it a
/// link; the final crumb in a trail is always rendered as the current page
/// regardless, so its `on_select` (if any) is ignored.
pub fn crumb<Msg>(label: impl Into<String>) -> Crumb<Msg> {
    Crumb {
        label: label.into(),
        icon: None,
        on_select: None,
        key: None,
    }
}

impl<Msg> Crumb<Msg> {
    /// A leading icon shown before the label (e.g. a home glyph on the root).
    #[must_use]
    pub fn icon(mut self, icon: impl Into<Element<Msg>>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Emits this message when the crumb is activated (click or Enter/Space).
    /// Ignored on the final crumb, which is the non-interactive current page.
    #[must_use]
    pub fn on_select(mut self, msg: Msg) -> Self {
        self.on_select = Some(msg);
        self
    }

    /// Stable identity key for reorderable contexts.
    #[must_use]
    pub fn id(mut self, key: &str) -> Self {
        self.key = Some(key.to_owned());
        self
    }
}

/// A breadcrumb trail under construction; converts into an [`Element`].
pub struct Breadcrumbs<Msg> {
    items: Vec<Crumb<Msg>>,
    max_items: Option<usize>,
}

/// A breadcrumb trail from a sequence of [`crumb`]s. Earlier crumbs with an
/// `.on_select` render as focusable links; the last is the bold, non-interactive
/// current page. Chevron separators sit between entries. Pair with
/// [`Breadcrumbs::max_items`] to collapse long trails behind an ellipsis.
pub fn breadcrumbs<Msg>(items: impl IntoIterator<Item = Crumb<Msg>>) -> Breadcrumbs<Msg> {
    Breadcrumbs {
        items: items.into_iter().collect(),
        max_items: None,
    }
}

impl<Msg> Breadcrumbs<Msg> {
    /// Collapse the middle of trails longer than `max` into a single `…`,
    /// keeping the root and the last `max - 1` crumbs visible. Values below 2
    /// are treated as 2 (root + current). No effect on shorter trails.
    #[must_use]
    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max);
        self
    }
}

/// A `›` chevron separator, muted and decorative.
fn separator<Msg>() -> Element<Msg> {
    icons::chevron_right()
        .w(14.0)
        .h(14.0)
        .shrink0()
        .themed(|t: &Theme, s| s.color(t.text_muted))
}

/// Lays out a crumb's optional icon and its label into one centered row, both
/// inked from the same theme token at the given weight.
fn body<Msg>(
    icon: Option<Element<Msg>>,
    label: String,
    weight: Weight,
    color: fn(&Theme) -> Color,
) -> Element<Msg> {
    let mut kids: Vec<Element<Msg>> = Vec::new();
    if let Some(icon) = icon {
        kids.push(
            icon.w(14.0)
                .h(14.0)
                .shrink0()
                .themed(move |t: &Theme, s| s.color(color(t))),
        );
    }
    kids.push(
        text(label)
            .size(TextSize::Sm)
            .weight(weight)
            .themed(move |t: &Theme, s| s.color(color(t))),
    );
    row().items_center().gap(SP1).shrink0().children(kids)
}

impl<Msg> From<Breadcrumbs<Msg>> for Element<Msg> {
    fn from(b: Breadcrumbs<Msg>) -> Self {
        let total = b.items.len();
        // (head_end, tail_start): indices in [head_end, tail_start) collapse to
        // one ellipsis, keeping the root and the last `max - 1` crumbs.
        let collapse = b
            .max_items
            .map(|m| m.max(2))
            .filter(|&m| total > m)
            .map(|m| (1usize, total - (m - 1)));

        let mut entries: Vec<Element<Msg>> = Vec::new();
        for (i, item) in b.items.into_iter().enumerate() {
            let is_last = i + 1 == total;
            if let Some((head_end, tail_start)) = collapse {
                if i == head_end {
                    entries.push(
                        text("…")
                            .size(TextSize::Sm)
                            .themed(|t: &Theme, s| s.color(t.text_muted)),
                    );
                }
                if i >= head_end && i < tail_start {
                    continue; // collapsed away
                }
            }

            let label_text = item.label.clone();
            if is_last || item.on_select.is_none() {
                // Current page (bold `t.text`) or a label-only crumb (muted).
                let weight = if is_last {
                    Weight::Medium
                } else {
                    Weight::Regular
                };
                let color: fn(&Theme) -> Color = if is_last {
                    |t| t.text
                } else {
                    |t| t.text_muted
                };
                let mut current = body(item.icon, item.label, weight, color).label(label_text);
                if is_last {
                    current = current.semantics(Semantics::Label);
                }
                entries.push(current);
            } else {
                // An ancestor link: muted ink, a hover/press state-layer veil,
                // focusable, emitting its message on activation.
                let mut link = body(item.icon, item.label, Weight::Regular, |t| t.text_muted)
                    .px(SP1)
                    .h(24.0)
                    .themed(|t: &Theme, s| s.rounded(t.radius.sm))
                    .state_layer(|t| t.text)
                    .transition(Transition::colors())
                    .focusable(true)
                    .cursor(Cursor::Pointer)
                    .semantics(Semantics::Button)
                    .label(label_text);
                if let Some(key) = &item.key {
                    link = link.id(key);
                }
                if let Some(msg) = item.on_select {
                    link = link.on_click(msg);
                }
                entries.push(link);
            }
        }

        // Weave chevron separators between the entries.
        let count = entries.len();
        let mut woven: Vec<Element<Msg>> = Vec::with_capacity(count.saturating_mul(2));
        for (i, e) in entries.into_iter().enumerate() {
            woven.push(e);
            if i + 1 < count {
                woven.push(separator());
            }
        }
        row().items_center().gap(SP1).children(woven)
    }
}
