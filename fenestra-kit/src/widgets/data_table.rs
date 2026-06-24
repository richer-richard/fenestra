//! Data table: sortable headers, multi-select, row virtualization, a sticky
//! header, column resize / reorder / pin (freeze), and a filter row — over the
//! plain [`crate::table`] look.
//!
//! Elm-pure: the table renders from app-owned state and only *emits*. The app
//! sorts, filters, reorders, resizes, and selects in `update`; the widget never
//! mutates. In particular the filter row does **not** filter rows — it emits
//! [`on_filter`](DataTable::on_filter) and the app passes the filtered
//! `rows` back in.
//!
//! Past a row threshold (or when a sticky header / pins are requested) the body
//! becomes a dedicated scroll container that virtualizes its rows, so a
//! 100k-row table stays a screenful of work per frame. Give it a stable
//! [`id`](DataTable::id) so the scroll offset and per-column filter editors
//! persist across frames.
//!
//! ```
//! use fenestra_kit::data_table;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Sort(usize),
//!     Resize(usize, f32),
//!     ResizeDone,
//!     Reorder(usize, usize),
//!     Filter(usize, String),
//! }
//!
//! let el: fenestra_core::Element<Msg> = data_table(
//!     ["Name", "Role", "Commits"],
//!     vec![vec!["Ripley".into(), "Officer".into(), "128".into()]],
//! )
//! .id("crew")
//! .column_widths([160.0, 200.0, 100.0])
//! .on_sort(Msg::Sort)
//! .on_resize(Msg::Resize)
//! .on_resize_end(Msg::ResizeDone)
//! .on_reorder(Msg::Reorder)
//! .on_filter(Msg::Filter)
//! .into();
//! ```

use std::rc::Rc;

use fenestra_core::{
    Cursor, Element, SP2, SP3, Semantics, TextSize, Theme, Track, Transition, Weight, col, div,
    raw_input, row, text,
};

use crate::checkbox;

/// Width of the leading multi-select checkbox column, in logical px.
const CHECK_COL: f32 = 44.0;
/// Fixed body-row and header height, in logical px (virtualization needs it).
const ROW_H: f32 = 34.0;
/// Smallest a column may be dragged.
const MIN_COL_W: f32 = 40.0;
/// Largest a column may be dragged.
const MAX_COL_W: f32 = 800.0;
/// Width of the trailing resize handle on each header cell.
const HANDLE_W: f32 = 8.0;
/// How close (px) a press must land to a column boundary to start a resize.
const HANDLE_HIT: f32 = 8.0;
/// Above this row count the body auto-virtualizes (a sticky header comes free).
const AUTO_SCROLL_ROWS: usize = 50;

/// A data table under construction; converts into an [`Element`].
pub struct DataTable<Msg> {
    columns: Vec<String>,
    rows: Rc<Vec<Vec<String>>>,
    sort: Option<(usize, bool)>,
    selected: Option<usize>,
    selection: Option<Rc<Vec<bool>>>,
    id: Option<String>,
    sticky_header: Option<bool>,
    column_widths: Option<Vec<f32>>,
    resize_active: Option<usize>,
    column_order: Option<Vec<usize>>,
    filter: Option<Vec<String>>,
    pinned_left: usize,
    pinned_right: usize,
    on_sort: Option<Rc<dyn Fn(usize) -> Msg>>,
    on_select: Option<Rc<dyn Fn(usize) -> Msg>>,
    on_select_row: Option<Rc<dyn Fn(usize) -> Msg>>,
    on_select_all: Option<Msg>,
    on_resize: Option<Rc<dyn Fn(usize, f32) -> Msg>>,
    on_resize_end: Option<Msg>,
    on_reorder: Option<Rc<dyn Fn(usize, usize) -> Msg>>,
    on_filter: Option<Rc<dyn Fn(usize, String) -> Msg>>,
}

/// A table whose headers sort and whose rows select. Pass rows in the order you
/// want them shown (sort/filter in `update` when the table emits); `sort` only
/// draws the ▲/▼ indicator.
pub fn data_table<Msg>(
    columns: impl IntoIterator<Item = impl Into<String>>,
    rows: Vec<Vec<String>>,
) -> DataTable<Msg> {
    DataTable {
        columns: columns.into_iter().map(Into::into).collect(),
        rows: Rc::new(rows),
        sort: None,
        selected: None,
        selection: None,
        id: None,
        sticky_header: None,
        column_widths: None,
        resize_active: None,
        column_order: None,
        filter: None,
        pinned_left: 0,
        pinned_right: 0,
        on_sort: None,
        on_select: None,
        on_select_row: None,
        on_select_all: None,
        on_resize: None,
        on_resize_end: None,
        on_reorder: None,
        on_filter: None,
    }
}

impl<Msg> DataTable<Msg> {
    /// Stable identity key. The virtualized body's scroll container uses
    /// `dt-body-{key}` and the filter editors `dt-filter-{key}-{col}`, so the
    /// scroll offset and caret state survive rebuilds. Set it whenever the
    /// table scrolls or filters.
    #[must_use]
    pub fn id(mut self, key: &str) -> Self {
        self.id = Some(key.to_owned());
        self
    }

    /// Draws the sort indicator: data-column index + ascending?. The active
    /// column's header (label and ▲/▼ caret) is tinted with the accent.
    #[must_use]
    pub fn sort(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
        self
    }

    /// Highlights one row by index.
    #[must_use]
    pub fn selected(mut self, row: Option<usize>) -> Self {
        self.selected = row;
        self
    }

    /// Adds a leading checkbox column for multi-select; `flags[i]` is row `i`'s
    /// selected state (app-owned — flip it in `update`). The header holds a
    /// tri-state select-all box: unchecked when none are selected, a mixed dash
    /// when some are, checked when all are. Selected rows also take the accent
    /// highlight.
    #[must_use]
    pub fn selection(mut self, flags: impl IntoIterator<Item = bool>) -> Self {
        self.selection = Some(Rc::new(flags.into_iter().collect()));
        self
    }

    /// Forces (`true`) or forbids (`false`) the scrolling body. The body is a
    /// scroll container — header outside, rows virtualized — whenever this is
    /// `true`, pins are set, or the row count exceeds an internal threshold.
    /// When it scrolls, the table fills its parent's height, so give it a
    /// bounded one.
    #[must_use]
    pub fn sticky_header(mut self, on: bool) -> Self {
        self.sticky_header = Some(on);
        self
    }

    /// Sets explicit per-data-column widths in logical px (parallel to the
    /// columns, *not* the display order). Columns become fixed `Px` tracks
    /// (clamped to 40..=800) instead of equal `Fr` shares; required for
    /// [`on_resize`](Self::on_resize) and column pinning, since both need known
    /// pixel boundaries.
    #[must_use]
    pub fn column_widths(mut self, widths: impl IntoIterator<Item = f32>) -> Self {
        self.column_widths = Some(widths.into_iter().collect());
        self
    }

    /// The data column currently being resized (app-owned). Set it from the
    /// first [`on_resize`](Self::on_resize) and clear it on
    /// [`on_resize_end`](Self::on_resize_end); the drag continues this column
    /// even when the pointer wanders off the boundary.
    #[must_use]
    pub fn resize_active(mut self, column: Option<usize>) -> Self {
        self.resize_active = column;
        self
    }

    /// Sets the display order as `display_index -> data_index` (a permutation
    /// of `0..columns`). Headers, filters, and cells all render in this order;
    /// sort and selection keep tracking the underlying *data* column. Ignored
    /// unless it is a valid permutation.
    #[must_use]
    pub fn column_order(mut self, order: impl IntoIterator<Item = usize>) -> Self {
        self.column_order = Some(order.into_iter().collect());
        self
    }

    /// Sets the current filter text per *data* column (parallel to the
    /// columns), rendered in a filter row between the header and body. The
    /// widget never filters rows itself — wire [`on_filter`](Self::on_filter),
    /// filter in `update`, and pass the surviving `rows` back in.
    #[must_use]
    pub fn filter(mut self, text: impl IntoIterator<Item = String>) -> Self {
        self.filter = Some(text.into_iter().collect());
        self
    }

    /// Freezes the first `count` *display* columns to the left edge during
    /// horizontal scroll (needs [`column_widths`](Self::column_widths)).
    #[must_use]
    pub fn pinned_left(mut self, count: usize) -> Self {
        self.pinned_left = count;
        self
    }

    /// Freezes the last `count` *display* columns to the right edge during
    /// horizontal scroll (needs [`column_widths`](Self::column_widths)).
    #[must_use]
    pub fn pinned_right(mut self, count: usize) -> Self {
        self.pinned_right = count;
        self
    }

    /// Maps a clicked header's *data*-column index to a message.
    #[must_use]
    pub fn on_sort(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_sort = Some(Rc::new(f));
        self
    }

    /// Maps a clicked row's index to a message.
    #[must_use]
    pub fn on_select(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Maps a toggled row checkbox's index to a message. Needs
    /// [`selection`](Self::selection) for the checkbox column to appear.
    #[must_use]
    pub fn on_select_row(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select_row = Some(Rc::new(f));
        self
    }

    /// The message the tri-state select-all header checkbox emits when toggled.
    /// Needs [`selection`](Self::selection) for the column to appear.
    #[must_use]
    pub fn on_select_all(mut self, msg: Msg) -> Self {
        self.on_select_all = Some(msg);
        self
    }

    /// Maps a resize drag to `(data_column, new_width)` (already clamped to
    /// 40..=800). Set [`resize_active`](Self::resize_active) to the column on
    /// the first event and store the width; needs
    /// [`column_widths`](Self::column_widths).
    #[must_use]
    pub fn on_resize(mut self, f: impl Fn(usize, f32) -> Msg + 'static) -> Self {
        self.on_resize = Some(Rc::new(f));
        self
    }

    /// The message emitted when a resize drag releases — clear
    /// [`resize_active`](Self::resize_active) here.
    #[must_use]
    pub fn on_resize_end(mut self, msg: Msg) -> Self {
        self.on_resize_end = Some(msg);
        self
    }

    /// Maps a header drag-and-drop to `(from_display, to_display)`. Apply the
    /// move to your [`column_order`](Self::column_order) in `update`. A plain
    /// click still sorts (a drop onto the same column is ignored).
    #[must_use]
    pub fn on_reorder(mut self, f: impl Fn(usize, usize) -> Msg + 'static) -> Self {
        self.on_reorder = Some(Rc::new(f));
        self
    }

    /// Maps a filter-cell edit to `(data_column, text)`.
    #[must_use]
    pub fn on_filter(mut self, f: impl Fn(usize, String) -> Msg + 'static) -> Self {
        self.on_filter = Some(Rc::new(f));
        self
    }
}

/// Shared, owned context handed to each (possibly virtualized) body row.
struct BodyCtx<Msg> {
    rows: Rc<Vec<Vec<String>>>,
    selection: Option<Rc<Vec<bool>>>,
    order: Vec<usize>,
    tracks: Vec<Track>,
    width: Option<f32>,
    selected: Option<usize>,
    select: bool,
    pinned_left: usize,
    pinned_right: usize,
    left_off: Vec<f32>,
    right_off: Vec<f32>,
    on_select: Option<Rc<dyn Fn(usize) -> Msg>>,
    on_select_row: Option<Rc<dyn Fn(usize) -> Msg>>,
}

impl<Msg: Clone + 'static> BodyCtx<Msg> {
    /// Pins a cell to the frozen panel: a sticky left/right offset plus an
    /// opaque elevated fill (so scrolling cells slide under it) and a hairline
    /// on the freeze edge.
    fn pin(&self, cell: Element<Msg>, d: usize) -> Element<Msg> {
        let cols = self.order.len();
        let mut cell = cell.themed(|th: &Theme, s| s.bg(th.neutrals.step(2)));
        if d < self.pinned_left {
            cell = cell.sticky_left(self.left_off[d]);
            if d + 1 == self.pinned_left {
                cell = cell.themed(|th: &Theme, s| s.border_right(1.0, th.border_subtle));
            }
        } else if d >= cols - self.pinned_right {
            cell = cell.sticky_right(self.right_off[d]);
            if d == cols - self.pinned_right {
                cell = cell.themed(|th: &Theme, s| s.border_left(1.0, th.border_subtle));
            }
        }
        cell
    }

    fn row(&self, i: usize) -> Element<Msg> {
        let cols = self.order.len();
        let checked = self
            .selection
            .as_ref()
            .and_then(|f| f.get(i).copied())
            .unwrap_or(false);

        let mut kids: Vec<Element<Msg>> = Vec::with_capacity(cols + 1);
        if self.select {
            let mut cb = checkbox(checked);
            if let Some(f) = &self.on_select_row {
                cb = cb.on_toggle(f(i));
            }
            let cb: Element<Msg> = cb.into();
            // Freeze the checkbox alongside a left-pinned panel.
            if self.pinned_left > 0 {
                kids.push(
                    row()
                        .items_center()
                        .h_full()
                        .sticky_left(SP3)
                        .themed(|th: &Theme, s| s.bg(th.neutrals.step(2)))
                        .child(cb),
                );
            } else {
                kids.push(cb);
            }
        }
        for d in 0..cols {
            let data = self.order[d];
            let value = self.rows[i].get(data).cloned().unwrap_or_default();
            let txt = text(value).size(TextSize::Sm).tabular();
            let pinned = d < self.pinned_left || d >= cols - self.pinned_right;
            if pinned {
                kids.push(self.pin(row().items_center().h_full().child(txt), d));
            } else {
                kids.push(txt);
            }
        }

        let mut r = row()
            .grid_cols(self.tracks.clone())
            .px(SP3)
            .h(ROW_H)
            .items_center()
            .shrink0()
            .transition(Transition::colors())
            .themed(move |th: &Theme, s| {
                if i % 2 == 1 {
                    s.bg(th.neutrals.step(1))
                } else {
                    s
                }
            })
            .children(kids);
        if let Some(w) = self.width {
            r = r.w(w);
        }
        if self.selected == Some(i) || checked {
            r = r.themed(|th: &Theme, s| s.bg(th.accent_bg));
        }
        if let Some(f) = &self.on_select {
            r = r
                .on_click(f(i))
                .cursor(Cursor::Pointer)
                .label(format!("row {i}"));
        }
        r
    }
}

/// Validates an app-supplied display order; falls back to identity unless it is
/// a true permutation of `0..cols`.
fn resolve_order(order: Option<&Vec<usize>>, cols: usize) -> Vec<usize> {
    if let Some(o) = order
        && o.len() == cols
    {
        let mut seen = vec![false; cols];
        for &d in o {
            if d >= cols || seen[d] {
                return (0..cols).collect();
            }
            seen[d] = true;
        }
        return o.clone();
    }
    (0..cols).collect()
}

impl<Msg: Clone + 'static> From<DataTable<Msg>> for Element<Msg> {
    fn from(t: DataTable<Msg>) -> Self {
        let cols = t.columns.len();
        let n_rows = t.rows.len();
        let select = t.selection.is_some();
        let lead = if select { CHECK_COL } else { 0.0 };
        let order = resolve_order(t.column_order.as_ref(), cols);

        // Px tracks when explicit widths are given (resize/pin need pixels),
        // else equal Fr shares — the responsive default.
        let widths_display: Option<Vec<f32>> = t.column_widths.as_ref().and_then(|w| {
            (w.len() == cols).then(|| {
                order
                    .iter()
                    .map(|&data| w[data].clamp(MIN_COL_W, MAX_COL_W))
                    .collect()
            })
        });
        let px_mode = widths_display.is_some();

        let mut tracks: Vec<Track> = match &widths_display {
            Some(wd) => wd.iter().map(|&w| Track::Px(w)).collect(),
            None => vec![Track::Fr(1.0); cols.max(1)],
        };
        if select {
            tracks.insert(0, Track::Px(CHECK_COL));
        }
        // The grid tracks live inside the rows' SP3 horizontal padding, so the
        // element is the track sum plus that padding on both edges. Sizing the
        // header/rows to this keeps `fraction_in` proportional to it (the
        // resize drag subtracts the leading SP3 to reach content coordinates).
        let total_w = widths_display
            .as_ref()
            .map(|wd| lead + wd.iter().sum::<f32>() + 2.0 * SP3);

        // Pins (display columns) only freeze with known pixel widths. Offsets
        // are canvas distances from the body's edge, so they include the SP3
        // padding the cells sit inside.
        let pl = if px_mode { t.pinned_left.min(cols) } else { 0 };
        let pr = if px_mode {
            t.pinned_right.min(cols - pl)
        } else {
            0
        };
        let has_pins = pl + pr > 0;
        let (mut left_off, mut right_off) = (vec![0.0; cols], vec![0.0; cols]);
        if let Some(wd) = &widths_display {
            let mut acc = SP3 + lead;
            for (lo, &w) in left_off.iter_mut().zip(wd) {
                *lo = acc;
                acc += w;
            }
            let mut acc = SP3;
            for (ro, &w) in right_off.iter_mut().zip(wd).rev() {
                *ro = acc;
                acc += w;
            }
        }

        let scrolled = t.sticky_header.unwrap_or(n_rows > AUTO_SCROLL_ROWS) || has_pins;

        let header = build_header(&t, &order, &tracks, &widths_display, lead, total_w);
        let filter_row = t
            .on_filter
            .as_ref()
            .filter(|_| t.filter.is_some())
            .map(|f| build_filter(&t, &order, &tracks, total_w, f.clone()));

        let ctx = Rc::new(BodyCtx {
            rows: t.rows.clone(),
            selection: t.selection.clone(),
            order,
            tracks,
            width: total_w,
            selected: t.selected,
            select,
            pinned_left: pl,
            pinned_right: pr,
            left_off,
            right_off,
            on_select: t.on_select.clone(),
            on_select_row: t.on_select_row.clone(),
        });

        let frame = || {
            col::<Msg>()
                .themed(|th: &Theme, s| s.rounded(th.radius.md))
                .overflow_hidden()
                .themed(|th: &Theme, s| s.border(1.0, th.border_subtle))
        };

        if !scrolled {
            // Inline: header + (filter) + rows as direct children. Each row keeps
            // a content-stable identity and animates its layout, so re-sorting
            // (or a filtered-out row above it) glides it into its new position
            // instead of jumping. Virtualized bodies recycle rows, so FLIP is
            // inline-only. Under reduced motion the slide snaps, so the resting
            // table is byte-identical to the classic small table.
            let mut kids: Vec<Element<Msg>> = Vec::with_capacity(n_rows + 2);
            kids.push(header);
            kids.extend(filter_row);
            kids.extend((0..n_rows).map(|i| {
                let key = ctx.rows[i].join("\u{1f}");
                ctx.row(i).id(&key).animate_layout()
            }));
            return frame().w_full().children(kids);
        }

        // Scrolled: the header (and filter) stay outside the body, so they
        // never scroll vertically — a sticky header for free. The body
        // virtualizes its rows; pins make it a 2D scroller so frozen columns
        // survive horizontal scroll.
        let body_id = format!("dt-body-{}", t.id.as_deref().unwrap_or("table"));
        let mut body = col::<Msg>().id(&body_id).w_full().h_full();
        body = if px_mode {
            body.scroll_xy()
        } else {
            body.scroll_y()
        };
        let body = body.virtual_rows(n_rows, ROW_H, move |i| ctx.row(i));

        let mut kids: Vec<Element<Msg>> = Vec::with_capacity(3);
        kids.push(header);
        kids.extend(filter_row);
        kids.push(body);
        frame().w_full().h_full().children(kids)
    }
}

/// Builds one header cell in display position `d` (data column `order[d]`),
/// preserving the classic structure when neither resize nor reorder is wired.
fn header_cell<Msg: Clone + 'static>(
    t: &DataTable<Msg>,
    d: usize,
    data: usize,
    has_handle: bool,
) -> Element<Msg> {
    let c = &t.columns[data];
    let active = matches!(t.sort, Some((col, _)) if col == data);
    let indicator = match t.sort {
        Some((col, true)) if col == data => " ▲",
        Some((col, false)) if col == data => " ▼",
        _ => "",
    };
    // Keep the caret in the same text leaf as the label so its accessible name
    // stays "<col> ▲/▼"; tint the active header accent.
    let mut label = text(format!("{c}{indicator}"))
        .size(TextSize::Sm)
        .weight(Weight::Semibold);
    if active {
        label = label.themed(|th: &Theme, s| s.color(th.accent));
    }

    // The interactive sort/reorder surface.
    let mut sort_area = if has_handle {
        row().grow().items_center().h_full().children([label])
    } else {
        row().items_center().h_full().children([label])
    };
    if let Some(f) = &t.on_sort {
        sort_area = sort_area
            .on_click(f(data))
            .cursor(Cursor::Pointer)
            .focusable(true)
            .semantics(Semantics::Button)
            .transition(Transition::colors())
            .state_layer(|th| th.text)
            .label(format!("sort by {c}"));
    }
    if t.on_reorder.is_some() {
        sort_area = sort_area.drag_source(format!("col-reorder:{d}"));
    }

    let mut cell = if has_handle {
        // The trailing handle is inert: pressing it falls through to the
        // header row's resize drag rather than the sort surface.
        let handle = div()
            .w(HANDLE_W)
            .h_full()
            .shrink0()
            .cursor(Cursor::Pointer)
            .hover_themed(|th: &Theme, s| s.bg(th.accent_bg));
        row().items_center().h_full().children([sort_area, handle])
    } else {
        sort_area
    };

    if let Some(reorder) = &t.on_reorder {
        let reorder = reorder.clone();
        cell = cell.on_drop(move |payload| {
            payload
                .strip_prefix("col-reorder:")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|&from| from != d)
                .map(|from| reorder(from, d))
        });
    }
    cell
}

/// Assembles the header row (select-all + cells) and wires the resize drag.
/// Pinned header cells inherit their alignment from the shared grid tracks, so
/// no pin counts are needed here.
fn build_header<Msg: Clone + 'static>(
    t: &DataTable<Msg>,
    order: &[usize],
    tracks: &[Track],
    widths_display: &Option<Vec<f32>>,
    lead: f32,
    total_w: Option<f32>,
) -> Element<Msg> {
    let has_handle = t.on_resize.is_some() && widths_display.is_some();

    let select_all: Option<Element<Msg>> = t.selection.as_ref().map(|flags| {
        let all = !flags.is_empty() && flags.iter().all(|&b| b);
        let mixed = !all && flags.iter().any(|&b| b);
        let mut cb = checkbox(all).indeterminate(mixed);
        if let Some(msg) = &t.on_select_all {
            cb = cb.on_toggle(msg.clone());
        }
        cb.into()
    });

    let mut kids: Vec<Element<Msg>> = Vec::with_capacity(order.len() + 1);
    kids.extend(select_all);
    kids.extend((0..order.len()).map(|d| header_cell(t, d, order[d], has_handle)));

    let mut header = row()
        .grid_cols(tracks.to_vec())
        .px(SP3)
        .h(ROW_H)
        .items_center()
        .shrink0()
        .themed(|th: &Theme, s| s.bg(th.neutrals.step(2)))
        .children(kids);
    if let Some(w) = total_w {
        header = header.w(w);
    }

    // Resize: one drag on the whole header row. `fraction_in` gives the press
    // as a fraction of the (content-width) header, so `fx * total` is a pixel
    // x; the boundary nearest it (or the active column) is what we resize.
    if let (Some(resize), Some(wd), Some(total)) = (&t.on_resize, widths_display, total_w)
        && has_handle
    {
        let resize = resize.clone();
        let wd = wd.clone();
        let ord = order.to_vec();
        let active = t.resize_active;
        header = header.on_drag(move |fx, _fy| {
            if total <= 0.0 {
                return None;
            }
            // `fx` spans the padded element; shift into content coordinates.
            let x = fx * total - SP3;
            let d = match active {
                Some(data_col) => ord.iter().position(|&c| c == data_col)?,
                None => {
                    let mut acc = lead;
                    let mut hit = None;
                    for (d, &w) in wd.iter().enumerate() {
                        acc += w;
                        if (x - acc).abs() <= HANDLE_HIT {
                            hit = Some(d);
                            break;
                        }
                    }
                    hit?
                }
            };
            let left: f32 = lead + wd[..d].iter().sum::<f32>();
            let new_w = (x - left).clamp(MIN_COL_W, MAX_COL_W);
            Some(resize(ord[d], new_w))
        });
        if let Some(end) = &t.on_resize_end {
            header = header.on_drag_end(end.clone());
        }
    }
    header
}

/// Builds the filter row: a compact input per column bound to `on_filter`.
fn build_filter<Msg: Clone + 'static>(
    t: &DataTable<Msg>,
    order: &[usize],
    tracks: &[Track],
    total_w: Option<f32>,
    on_filter: Rc<dyn Fn(usize, String) -> Msg>,
) -> Element<Msg> {
    let filter = t.filter.as_ref();
    let mut kids: Vec<Element<Msg>> = Vec::with_capacity(order.len() + 1);
    if t.selection.is_some() {
        kids.push(div());
    }
    for &data in order {
        let value = filter
            .and_then(|f| f.get(data))
            .cloned()
            .unwrap_or_default();
        let f = on_filter.clone();
        // `grow` fills the cell (a grid item with an explicit width would size
        // against the container, not the track, and overflow).
        let mut input = raw_input(value, "Filter")
            .grow()
            .h(26.0)
            .px(SP2)
            .size(TextSize::Sm)
            .themed(|th: &Theme, s| s.rounded(th.radius.sm))
            .transition(Transition::colors())
            .themed(|th: &Theme, s| s.bg(th.surface_raised).border(1.0, th.border))
            .hover_themed(|th: &Theme, s| s.border(1.0, th.border_strong))
            .focus_themed(|th: &Theme, s| s.border(1.0, th.accent))
            .label(format!("filter {}", t.columns[data]))
            .on_input(move |s| f(data, s.to_owned()));
        if let Some(key) = &t.id {
            input = input.id(&format!("dt-filter-{key}-{data}"));
        }
        // A 1px inset keeps adjacent inputs off each other while the cell still
        // stretches to its grid track, so columns line up with the header.
        // `min_w(0)` overrides the grid item's content floor so `1fr` tracks
        // stay equal (otherwise the inputs' intrinsic width overflows them).
        kids.push(
            row()
                .h_full()
                .items_center()
                .px(1.0)
                .min_w(0.0)
                .child(input),
        );
    }

    // No grid gap: the header and body have none, so the filter columns must
    // align with them track-for-track (the inset above gives breathing room).
    let mut filter_row = row()
        .grid_cols(tracks.to_vec())
        .px(SP3)
        .py(SP2)
        .items_center()
        .shrink0()
        .themed(|th: &Theme, s| s.bg(th.bg).border_bottom(1.0, th.border_subtle))
        .children(kids);
    if let Some(w) = total_w {
        filter_row = filter_row.w(w);
    }
    filter_row
}
