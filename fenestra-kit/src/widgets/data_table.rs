//! Data table: sortable headers and row selection over the plain
//! [`crate::table`] look. The app owns sort order and selection — the
//! table only *emits* (Elm-pure; sort your rows in `update`).

use fenestra_core::{
    Cursor, Element, SP3, Semantics, TextSize, Theme, Track, Transition, Weight, col, row, text,
};

use crate::checkbox;

/// Width of the leading multi-select checkbox column, in logical px.
const CHECK_COL: f32 = 44.0;

/// A data table under construction; converts into an [`Element`].
pub struct DataTable<Msg> {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    sort: Option<(usize, bool)>,
    selected: Option<usize>,
    selection: Option<Vec<bool>>,
    on_sort: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_select: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_select_row: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_select_all: Option<Msg>,
}

/// A table whose headers sort and whose rows select. Pass rows in the
/// order you want them shown (sort in `update` when `on_sort` fires);
/// `sort` only draws the ▲/▼ indicator.
pub fn data_table<Msg>(
    columns: impl IntoIterator<Item = impl Into<String>>,
    rows: Vec<Vec<String>>,
) -> DataTable<Msg> {
    DataTable {
        columns: columns.into_iter().map(Into::into).collect(),
        rows,
        sort: None,
        selected: None,
        selection: None,
        on_sort: None,
        on_select: None,
        on_select_row: None,
        on_select_all: None,
    }
}

impl<Msg> DataTable<Msg> {
    /// Draws the sort indicator: column index + ascending?. The active
    /// column's header (label and ▲/▼ caret) is tinted with the accent.
    #[must_use]
    pub fn sort(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
        self
    }

    /// Highlights one row.
    #[must_use]
    pub fn selected(mut self, row: Option<usize>) -> Self {
        self.selected = row;
        self
    }

    /// Adds a leading checkbox column for multi-select; `flags[i]` is row
    /// `i`'s selected state (app-owned — flip it in `update`). The header
    /// holds a tri-state select-all box: unchecked when none are selected, a
    /// mixed dash when some are, checked when all are. Selected rows also take
    /// the accent highlight.
    #[must_use]
    pub fn selection(mut self, flags: impl IntoIterator<Item = bool>) -> Self {
        self.selection = Some(flags.into_iter().collect());
        self
    }

    /// Maps a clicked header's column index to a message.
    pub fn on_sort(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_sort = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps a clicked row's index to a message.
    pub fn on_select(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select = Some(std::rc::Rc::new(f));
        self
    }

    /// Maps a toggled row checkbox's index to a message. Needs
    /// [`selection`](Self::selection) for the checkbox column to appear.
    pub fn on_select_row(mut self, f: impl Fn(usize) -> Msg + 'static) -> Self {
        self.on_select_row = Some(std::rc::Rc::new(f));
        self
    }

    /// The message the tri-state select-all header checkbox emits when
    /// toggled. Needs [`selection`](Self::selection) for the column to appear.
    pub fn on_select_all(mut self, msg: Msg) -> Self {
        self.on_select_all = Some(msg);
        self
    }
}

impl<Msg: Clone + 'static> From<DataTable<Msg>> for Element<Msg> {
    fn from(t: DataTable<Msg>) -> Self {
        let n = t.columns.len().max(1);
        // A leading fixed checkbox column appears once `selection` is set.
        let select = t.selection.is_some();
        let tracks = || {
            let mut v = vec![Track::Fr(1.0); n];
            if select {
                v.insert(0, Track::Px(CHECK_COL));
            }
            v
        };

        // The tri-state select-all box, derived from the per-row flags.
        let select_all: Option<Element<Msg>> = t.selection.as_ref().map(|flags| {
            let all = !flags.is_empty() && flags.iter().all(|&b| b);
            let mixed = !all && flags.iter().any(|&b| b);
            let mut cb = checkbox(all).indeterminate(mixed);
            if let Some(msg) = &t.on_select_all {
                cb = cb.on_toggle(msg.clone());
            }
            cb.into()
        });

        let header_cells = t.columns.iter().enumerate().map(|(i, c)| {
            // Keep the caret in the same text leaf as the label so its
            // accessible name stays "<col> ▲/▼"; tint the active header accent.
            let active = matches!(t.sort, Some((col, _)) if col == i);
            let indicator = match t.sort {
                Some((col, true)) if col == i => " ▲",
                Some((col, false)) if col == i => " ▼",
                _ => "",
            };
            let mut label = text(format!("{c}{indicator}"))
                .size(TextSize::Sm)
                .weight(Weight::Semibold);
            if active {
                label = label.themed(|th: &Theme, s| s.color(th.accent));
            }
            let mut cell = row().items_center().h_full().children([label]);
            if let Some(f) = &t.on_sort {
                cell = cell
                    .on_click(f(i))
                    .cursor(Cursor::Pointer)
                    .focusable(true)
                    .semantics(Semantics::Button)
                    .transition(Transition::colors())
                    .state_layer(|th| th.text)
                    .label(format!("sort by {c}"));
            }
            cell
        });

        let mut header_kids: Vec<Element<Msg>> = Vec::with_capacity(n + 1);
        header_kids.extend(select_all);
        header_kids.extend(header_cells);

        let header = row()
            .grid_cols(tracks())
            .px(SP3)
            .h(34.0)
            .items_center()
            .shrink0()
            .themed(|th: &Theme, s| s.bg(th.neutrals.step(2)))
            .children(header_kids);

        let body = t.rows.iter().enumerate().map(|(i, cells)| {
            let checked = t
                .selection
                .as_ref()
                .and_then(|f| f.get(i).copied())
                .unwrap_or(false);

            let mut kids: Vec<Element<Msg>> = Vec::with_capacity(n + 1);
            if select {
                let mut cb = checkbox(checked);
                if let Some(f) = &t.on_select_row {
                    cb = cb.on_toggle(f(i));
                }
                kids.push(cb.into());
            }
            kids.extend(
                cells
                    .iter()
                    .map(|cell| text(cell.clone()).size(TextSize::Sm).tabular()),
            );

            let mut r = row()
                .grid_cols(tracks())
                .px(SP3)
                .h(34.0)
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
            // Either the single-row highlight or a ticked multi-select row.
            if t.selected == Some(i) || checked {
                r = r.themed(|th: &Theme, s| s.bg(th.accent_bg));
            }
            if let Some(f) = &t.on_select {
                r = r
                    .on_click(f(i))
                    .cursor(Cursor::Pointer)
                    .label(format!("row {i}"));
            }
            r
        });

        col()
            .w_full()
            .themed(|t: &Theme, s| s.rounded(t.radius.md))
            .overflow_hidden()
            .themed(|th: &Theme, s| s.border(1.0, th.border_subtle))
            .child(header)
            .children(body)
    }
}
