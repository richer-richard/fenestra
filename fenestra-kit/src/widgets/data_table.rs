//! Data table: sortable headers and row selection over the plain
//! [`crate::table`] look. The app owns sort order and selection — the
//! table only *emits* (Elm-pure; sort your rows in `update`).

use fenestra_core::{
    Cursor, Element, SP3, Semantics, TextSize, Theme, Track, Transition, Weight, col, row, text,
};

/// A data table under construction; converts into an [`Element`].
pub struct DataTable<Msg> {
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
    sort: Option<(usize, bool)>,
    selected: Option<usize>,
    on_sort: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
    on_select: Option<std::rc::Rc<dyn Fn(usize) -> Msg>>,
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
        on_sort: None,
        on_select: None,
    }
}

impl<Msg> DataTable<Msg> {
    /// Draws the sort indicator: column index + ascending?.
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
}

impl<Msg: Clone + 'static> From<DataTable<Msg>> for Element<Msg> {
    fn from(t: DataTable<Msg>) -> Self {
        let n = t.columns.len().max(1);
        let tracks = || vec![Track::Fr(1.0); n];

        let header = row()
            .grid_cols(tracks())
            .px(SP3)
            .h(34.0)
            .items_center()
            .shrink0()
            .themed(|th: &Theme, s| s.bg(th.neutrals.step(2)))
            .children(t.columns.iter().enumerate().map(|(i, c)| {
                let indicator = match t.sort {
                    Some((col, true)) if col == i => " ▲",
                    Some((col, false)) if col == i => " ▼",
                    _ => "",
                };
                let mut cell =
                    row()
                        .items_center()
                        .h_full()
                        .children([text(format!("{c}{indicator}"))
                            .size(TextSize::Sm)
                            .weight(Weight::Semibold)]);
                if let Some(f) = &t.on_sort {
                    cell = cell
                        .on_click(f(i))
                        .cursor(Cursor::Pointer)
                        .focusable(true)
                        .semantics(Semantics::Button)
                        .label(format!("sort by {c}"));
                }
                cell
            }));

        let body = t.rows.iter().enumerate().map(|(i, cells)| {
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
                .children(
                    cells
                        .iter()
                        .map(|cell| text(cell.clone()).size(TextSize::Sm).tabular()),
                );
            if t.selected == Some(i) {
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
