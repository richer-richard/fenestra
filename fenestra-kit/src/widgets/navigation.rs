//! Navigation widgets: [`breadcrumbs`] (a trail of links to ancestor pages with
//! a current-page marker and overflow collapse) and [`pagination`] (a numbered
//! page strip with prev/next arrows and ellipsis overflow).
//!
//! ```
//! use fenestra_kit::{breadcrumbs, crumb, pagination};
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Go(usize),
//!     Page(usize),
//! }
//!
//! let trail: fenestra_core::Element<Msg> = breadcrumbs([
//!     crumb("Home").on_select(Msg::Go(0)),
//!     crumb("Library").on_select(Msg::Go(1)),
//!     crumb("Charts"), // the last crumb is always the current page
//! ])
//! .into();
//!
//! let pages: fenestra_core::Element<Msg> = pagination(6, 20).on_select(Msg::Page).into();
//! ```

use fenestra_core::{
    Color, Cursor, Element, SP1, SP2, SP3, Semantics, TextSize, Theme, Transition, Weight, col,
    div, row, text,
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

/// A 36×36 page cell: the current page is a solid accent chip; the rest are
/// ghost cells with a state-layer hover that emit their message on activation.
fn page_cell<Msg>(n: usize, current: bool, msg: Option<Msg>) -> Element<Msg> {
    let label = n.to_string();
    let mut cell = row()
        .items_center()
        .justify_center()
        .min_w(36.0)
        .h(36.0)
        .px(SP2)
        .shrink0()
        .themed(|t: &Theme, s| s.rounded(t.radius.md))
        .transition(Transition::colors())
        .focusable(true)
        .cursor(Cursor::Pointer)
        .semantics(Semantics::Button)
        .label(if current {
            format!("Page {label}, current")
        } else {
            format!("Go to page {label}")
        })
        .children([text(label)
            .size(TextSize::Sm)
            .tabular()
            .weight(if current {
                Weight::Medium
            } else {
                Weight::Regular
            })
            .themed(move |t: &Theme, s| s.color(if current { t.on_accent } else { t.text }))]);
    if current {
        cell = cell.themed(|t: &Theme, s| s.bg(t.accent));
    } else {
        cell = cell.press_scale().state_layer(|t| t.text);
        if let Some(msg) = msg {
            cell = cell.on_click(msg);
        }
    }
    cell
}

/// A prev/next arrow cell; dimmed and inert at the boundary.
fn arrow_cell<Msg>(
    icon: Element<Msg>,
    label: &str,
    enabled: bool,
    msg: Option<Msg>,
) -> Element<Msg> {
    let mut cell = row()
        .items_center()
        .justify_center()
        .w(36.0)
        .h(36.0)
        .shrink0()
        .themed(|t: &Theme, s| s.rounded(t.radius.md))
        .transition(Transition::colors())
        .semantics(Semantics::Button)
        .label(label.to_owned())
        .children([icon
            .w(16.0)
            .h(16.0)
            .themed(move |t: &Theme, s| s.color(if enabled { t.text } else { t.text_disabled }))]);
    if enabled {
        cell = cell
            .press_scale()
            .state_layer(|t| t.text)
            .focusable(true)
            .cursor(Cursor::Pointer);
        if let Some(msg) = msg {
            cell = cell.on_click(msg);
        }
    } else {
        cell = cell.disabled(true);
    }
    cell
}

/// A muted ellipsis cell standing in for a run of hidden pages.
fn gap_cell<Msg>() -> Element<Msg> {
    row()
        .items_center()
        .justify_center()
        .min_w(36.0)
        .h(36.0)
        .shrink0()
        .children([text("…")
            .size(TextSize::Sm)
            .themed(|t: &Theme, s| s.color(t.text_muted))])
}

/// A pagination strip under construction; converts into an [`Element`].
pub struct Pagination<Msg> {
    page: usize,
    count: usize,
    siblings: usize,
    on_select: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
}

/// A numbered pagination strip for `count` pages with `page` (1-based) current.
/// Always shows the first and last page plus a window of [`Pagination::siblings`]
/// pages on each side of the current one; runs hidden between those collapse to
/// an ellipsis. Prev/next arrows flank the numbers and disable at the ends. Wire
/// [`Pagination::on_select`] to receive the chosen page.
pub fn pagination<Msg>(page: usize, count: usize) -> Pagination<Msg> {
    Pagination {
        page,
        count,
        siblings: 1,
        on_select: None,
    }
}

impl<Msg> Pagination<Msg> {
    /// Page numbers kept on each side of the current page before collapsing to
    /// an ellipsis (default 1).
    #[must_use]
    pub fn siblings(mut self, n: usize) -> Self {
        self.siblings = n;
        self
    }

    /// Emits `f(page)` when a page number or arrow is activated (pages are
    /// 1-based; the arrows resolve to current ∓ 1).
    #[must_use]
    pub fn on_select(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

/// The widest sibling window a pagination strip will honor. The strip shows at
/// most `2 * siblings + 1` numbers around the current page; real pagers use one
/// or two, so 50 (up to 101 numbers) is already far beyond any legitimate use
/// and caps a hostile value before it can balloon the rendered strip into an
/// out-of-memory allocation.
const MAX_PAGINATION_SIBLINGS: usize = 50;

impl<Msg> From<Pagination<Msg>> for Element<Msg> {
    fn from(p: Pagination<Msg>) -> Self {
        // The strip only ever materializes the first page, the last page, and
        // the `siblings`-wide window around the current one — never a cell per
        // page — so the rendered size is bounded by `siblings` alone. Clamp only
        // that (and saturate the window math) to keep a hostile `siblings`/`page`
        // from ballooning the strip or overflowing `usize`; `count` carries no
        // allocation cost and is left uncapped so large pagers stay addressable.
        let count = p.count.max(1);
        let page = p.page.clamp(1, count);
        let siblings = p.siblings.min(MAX_PAGINATION_SIBLINGS);
        let f = p.on_select;
        let emit = |n: usize| f.as_ref().map(|f| f(n));

        // The visible page numbers: first, last, and a window around current.
        // `count` is deliberately uncapped (see above), so `page` can still
        // reach `usize::MAX`; saturating arithmetic keeps `page + siblings`
        // from overflowing even though `page`/`siblings` are already clamped.
        let lo = page.saturating_sub(siblings).max(1);
        let hi = page.saturating_add(siblings).min(count);
        let mut shown: Vec<usize> = vec![1, count];
        shown.extend(lo..=hi);
        shown.sort_unstable();
        shown.dedup();

        let prev_msg = if page > 1 { emit(page - 1) } else { None };
        let next_msg = if page < count { emit(page + 1) } else { None };

        let mut cells: Vec<Element<Msg>> = Vec::new();
        cells.push(arrow_cell(
            icons::chevron_left(),
            "Previous page",
            page > 1,
            prev_msg,
        ));
        let mut prev_n = 0usize;
        for n in shown {
            if prev_n != 0 && n > prev_n + 1 {
                cells.push(gap_cell());
            }
            let current = n == page;
            cells.push(page_cell(n, current, if current { None } else { emit(n) }));
            prev_n = n;
        }
        cells.push(arrow_cell(
            icons::chevron_right(),
            "Next page",
            page < count,
            next_msg,
        ));

        row().items_center().gap(SP1).children(cells)
    }
}

/// Where a step sits relative to the current one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StepState {
    Done,
    Active,
    Upcoming,
}

/// One step's definition: a title and an optional one-line description.
struct StepDef {
    title: String,
    desc: Option<String>,
    key: Option<String>,
}

/// The 28px status disc: an accent fill with a check (done) or number
/// (active), or a bordered surface with a muted number (upcoming).
fn step_disc<Msg>(state: StepState, number: usize) -> Element<Msg> {
    let inner: Element<Msg> = if state == StepState::Done {
        icons::check()
            .w(16.0)
            .h(16.0)
            .themed(|t: &Theme, s| s.color(t.on_accent))
    } else {
        text(number.to_string())
            .size(TextSize::Xs)
            .tabular()
            .weight(Weight::Medium)
            .themed(move |t: &Theme, s| {
                s.color(if state == StepState::Upcoming {
                    t.text_muted
                } else {
                    t.on_accent
                })
            })
    };
    let disc = row()
        .items_center()
        .justify_center()
        .w(28.0)
        .h(28.0)
        .shrink0()
        .rounded(14.0)
        .children([inner]);
    match state {
        StepState::Done | StepState::Active => disc.themed(|t: &Theme, s| s.bg(t.accent)),
        StepState::Upcoming => {
            disc.themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.5, t.border))
        }
    }
}

/// The grow-to-fill rule between two discs: accent once the left step is done.
fn step_connector<Msg>(done: bool) -> Element<Msg> {
    div()
        .h(2.0)
        .grow()
        .rounded(1.0)
        .themed(move |t: &Theme, s| s.bg(if done { t.accent } else { t.border }))
}

/// A horizontal stepper under construction; converts into an [`Element`].
pub struct Stepper<Msg> {
    current: usize,
    steps: Vec<StepDef>,
    on_select: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
}

/// A horizontal step indicator for a multi-step flow, with `current` the
/// 0-based active step. Steps before it render done (accent disc + check),
/// the active one shows its number in an accent disc with a bold label, and
/// later steps are muted. Grow-to-fill connectors join the discs and turn
/// accent as the flow advances. Add steps with [`Stepper::step`] /
/// [`Stepper::step_with`]; wire [`Stepper::on_select`] to make done and active
/// steps clickable.
pub fn stepper<Msg>(current: usize) -> Stepper<Msg> {
    Stepper {
        current,
        steps: Vec::new(),
        on_select: None,
    }
}

impl<Msg> Stepper<Msg> {
    /// Appends a step with just a title.
    #[must_use]
    pub fn step(mut self, title: impl Into<String>) -> Self {
        self.steps.push(StepDef {
            title: title.into(),
            desc: None,
            key: None,
        });
        self
    }

    /// Appends a step with a title and a one-line description.
    #[must_use]
    pub fn step_with(mut self, title: impl Into<String>, desc: impl Into<String>) -> Self {
        self.steps.push(StepDef {
            title: title.into(),
            desc: Some(desc.into()),
            key: None,
        });
        self
    }

    /// Sets a stable identity key on the most recently added step.
    #[must_use]
    pub fn step_id(mut self, key: &str) -> Self {
        if let Some(last) = self.steps.last_mut() {
            last.key = Some(key.to_owned());
        }
        self
    }

    /// Emits `f(index)` (0-based) when a done or active step is activated.
    #[must_use]
    pub fn on_select(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }
}

impl<Msg> From<Stepper<Msg>> for Element<Msg> {
    fn from(s: Stepper<Msg>) -> Self {
        let current = s.current;
        let total = s.steps.len();
        let f = s.on_select;

        let mut band: Vec<Element<Msg>> = Vec::with_capacity(total.saturating_mul(2));
        for (i, def) in s.steps.into_iter().enumerate() {
            let state = match i.cmp(&current) {
                std::cmp::Ordering::Less => StepState::Done,
                std::cmp::Ordering::Equal => StepState::Active,
                std::cmp::Ordering::Greater => StepState::Upcoming,
            };

            let title_color: fn(&Theme) -> Color = if state == StepState::Upcoming {
                |t| t.text_muted
            } else {
                |t| t.text
            };
            let title = text(def.title.clone())
                .size(TextSize::Sm)
                .weight(if state == StepState::Active {
                    Weight::Medium
                } else {
                    Weight::Regular
                })
                .themed(move |t: &Theme, s| s.color(title_color(t)));
            let mut label_kids: Vec<Element<Msg>> = vec![title];
            if let Some(desc) = def.desc {
                label_kids.push(
                    text(desc)
                        .size(TextSize::Xs)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                );
            }
            let label = col().gap(1.0).children(label_kids);

            let mut group = row()
                .items_center()
                .gap(SP2)
                .shrink0()
                .px(SP2)
                .py(SP1)
                .children([step_disc(state, i + 1), label])
                .label(def.title);
            // Done/active steps are navigable when a handler is wired.
            if state != StepState::Upcoming
                && let Some(f) = &f
            {
                group = group
                    .themed(|t: &Theme, s| s.rounded(t.radius.md))
                    .state_layer(|t| t.text)
                    .transition(Transition::colors())
                    .focusable(true)
                    .cursor(Cursor::Pointer)
                    .semantics(Semantics::Button)
                    .on_click(f(i));
                if let Some(key) = &def.key {
                    group = group.id(key);
                }
            }
            band.push(group);
            if i + 1 < total {
                band.push(step_connector(i < current));
            }
        }

        row().items_center().gap(SP3).w_full().children(band)
    }
}
