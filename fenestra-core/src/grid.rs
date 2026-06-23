//! Named grid lines and `grid-template-areas` — the second responsive-grid
//! layer over the numeric track core.
//!
//! taffy places grid items by numeric line, so fenestra resolves names to
//! 1-based line numbers itself, deterministically, before layout runs. A grid
//! container's [`grid_template_areas`](crate::Style::grid_template_areas) and
//! explicit line names ([`grid_col_names`](crate::Style::grid_col_names)) build a
//! [`ResolvedGrid`]; each child's [`grid_area`](crate::Style::grid_area) or named
//! line placement ([`grid_col_lines`](crate::Style::grid_col_lines)) resolves
//! against it. A name that does not resolve falls back to the child's own numeric
//! placement, so an unknown area never panics — it simply lays out `auto`.

use std::collections::HashMap;

use crate::style::{GridLines, GridPlace, Style};

/// A grid container's resolved line-name tables, built once per frame from its
/// style and shared by all of its children for placement. Each name maps to the
/// first 1-based grid line carrying it (CSS resolves a bare line name to its
/// first occurrence).
#[derive(Debug, Clone, Default)]
pub(crate) struct ResolvedGrid {
    col_lines: HashMap<String, i16>,
    row_lines: HashMap<String, i16>,
}

/// Resolves a grid container's name tables, or `None` when it declares neither
/// areas nor explicit line names (the overwhelmingly common case — children then
/// skip name resolution entirely).
pub(crate) fn resolve(style: &Style) -> Option<ResolvedGrid> {
    if style.grid_template_areas.is_empty()
        && style.grid_col_line_names.is_empty()
        && style.grid_row_line_names.is_empty()
    {
        return None;
    }
    let mut grid = ResolvedGrid::default();
    insert_line_names(&mut grid.col_lines, &style.grid_col_line_names);
    insert_line_names(&mut grid.row_lines, &style.grid_row_line_names);
    for (name, rect) in area_rects(&style.grid_template_areas) {
        // An area implicitly names its bounding lines `<name>-start` /
        // `<name>-end` on both axes (CSS named-area lines).
        grid.col_lines
            .entry(format!("{name}-start"))
            .or_insert(rect.col_start);
        grid.col_lines
            .entry(format!("{name}-end"))
            .or_insert(rect.col_end);
        grid.row_lines
            .entry(format!("{name}-start"))
            .or_insert(rect.row_start);
        grid.row_lines
            .entry(format!("{name}-end"))
            .or_insert(rect.row_end);
    }
    Some(grid)
}

/// Positional line names: the i-th entry labels the (i+1)-th grid line. Keeps the
/// first line for a repeated name.
fn insert_line_names(out: &mut HashMap<String, i16>, names: &[Vec<String>]) {
    for (i, line) in names.iter().enumerate() {
        for name in line {
            out.entry(name.clone()).or_insert(line_no(i));
        }
    }
}

/// Resolves a child's named placement against its parent [`ResolvedGrid`],
/// returning numeric column and row placements. Falls back to the child's own
/// numeric `grid_column` / `grid_row` for any axis it does not name.
pub(crate) fn place(child: &Style, grid: &ResolvedGrid) -> (GridPlace, GridPlace) {
    // `grid_area: name` is shorthand for the area's `-start` / `-end` lines on
    // both axes.
    if let Some(area) = &child.grid_area {
        let col = axis_from_lines(
            grid.col_lines.get(&format!("{area}-start")).copied(),
            grid.col_lines.get(&format!("{area}-end")).copied(),
        );
        let row = axis_from_lines(
            grid.row_lines.get(&format!("{area}-start")).copied(),
            grid.row_lines.get(&format!("{area}-end")).copied(),
        );
        return (
            col.unwrap_or(child.grid_column),
            row.unwrap_or(child.grid_row),
        );
    }
    let col = named_axis(&child.grid_column_lines, &grid.col_lines).unwrap_or(child.grid_column);
    let row = named_axis(&child.grid_row_lines, &grid.row_lines).unwrap_or(child.grid_row);
    (col, row)
}

/// The (rows, cols) cell dimensions of an area map — the implicit grid size when
/// the author gives `grid-template-areas` but no explicit tracks.
pub(crate) fn area_dims(areas: &[Vec<Option<String>>]) -> (usize, usize) {
    let rows = areas.len();
    let cols = areas.iter().map(Vec::len).max().unwrap_or(0);
    (rows, cols)
}

/// Resolves a [`GridLines`] (start/end names) against a line table, or `None`
/// when it names nothing resolvable.
fn named_axis(lines: &GridLines, table: &HashMap<String, i16>) -> Option<GridPlace> {
    let start = lines.start.as_ref().and_then(|n| table.get(n).copied());
    let end = lines.end.as_ref().and_then(|n| table.get(n).copied());
    if start.is_none() && end.is_none() {
        return None;
    }
    axis_from_lines(start, end)
}

/// Builds a [`GridPlace`] from optional start/end line numbers: a start with a
/// greater end spans the gap; a start alone spans one track; an end alone places
/// at that line.
fn axis_from_lines(start: Option<i16>, end: Option<i16>) -> Option<GridPlace> {
    match (start, end) {
        (Some(s), Some(e)) if e > s => Some(GridPlace {
            start: Some(s),
            span: u16::try_from(e - s).ok().filter(|n| *n > 1),
        }),
        (Some(s), _) => Some(GridPlace {
            start: Some(s),
            span: None,
        }),
        (None, Some(e)) => Some(GridPlace {
            start: Some(e),
            span: None,
        }),
        (None, None) => None,
    }
}

/// A resolved area's bounding lines, 1-based (taffy line numbers).
struct AreaRect {
    row_start: i16,
    row_end: i16,
    col_start: i16,
    col_end: i16,
}

/// Computes each named area's bounding rectangle in 1-based grid lines, keeping
/// only well-formed rectangular areas. CSS requires an area's cells to form a
/// solid rectangle; a non-rectangular name is dropped (lenient), so its children
/// fall back to auto placement rather than producing a broken layout.
fn area_rects(areas: &[Vec<Option<String>>]) -> Vec<(String, AreaRect)> {
    // name -> [min_row, max_row, min_col, max_col, cell_count] in 0-based cells.
    let mut bounds: HashMap<String, [usize; 5]> = HashMap::new();
    for (r, row) in areas.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let Some(name) = cell else { continue };
            let e = bounds.entry(name.clone()).or_insert([r, r, c, c, 0]);
            e[0] = e[0].min(r);
            e[1] = e[1].max(r);
            e[2] = e[2].min(c);
            e[3] = e[3].max(c);
            e[4] += 1;
        }
    }
    let mut out = Vec::with_capacity(bounds.len());
    for (name, [r0, r1, c0, c1, count]) in bounds {
        // Rectangular iff every cell in the bounding box carries this name.
        if (r1 - r0 + 1) * (c1 - c0 + 1) != count {
            continue;
        }
        out.push((
            name,
            AreaRect {
                row_start: line_no(r0),
                row_end: line_no(r1 + 1),
                col_start: line_no(c0),
                col_end: line_no(c1 + 1),
            },
        ));
    }
    out
}

/// 0-based cell/track index → its 1-based starting grid line number.
fn line_no(index: usize) -> i16 {
    i16::try_from(index + 1).unwrap_or(i16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Style;

    fn areas(rows: &[&str]) -> Style {
        Style::default().grid_template_areas(rows.iter().copied())
    }

    #[test]
    fn area_resolves_to_lines_on_both_axes() {
        let parent = areas(&["header header", "nav main"]);
        let grid = resolve(&parent).expect("has areas");
        // `header` spans both columns of row 1: cols 1..3, rows 1..2.
        let (col, row) = place(&Style::default().grid_area("header"), &grid);
        assert_eq!(col.start, Some(1));
        assert_eq!(col.span, Some(2));
        assert_eq!(row.start, Some(1));
        assert_eq!(row.span, None);
        // `main` is the bottom-right single cell: col 2, row 2.
        let (col, row) = place(&Style::default().grid_area("main"), &grid);
        assert_eq!(col.start, Some(2));
        assert_eq!(col.span, None);
        assert_eq!(row.start, Some(2));
    }

    #[test]
    fn dot_is_an_empty_cell() {
        let parent = areas(&["logo .", ". main"]);
        let grid = resolve(&parent).expect("has areas");
        let (col, row) = place(&Style::default().grid_area("main"), &grid);
        assert_eq!((col.start, row.start), (Some(2), Some(2)));
    }

    #[test]
    fn non_rectangular_area_is_dropped() {
        // `bad` is an L-shape (not a rectangle): dropped, so placement falls back.
        let parent = areas(&["bad bad", "bad ."]);
        let grid = resolve(&parent).expect("has areas");
        let fallback = Style::default().grid_area("bad").grid_col(7, 1);
        let (col, _row) = place(&fallback, &grid);
        assert_eq!(col.start, Some(7), "unresolved area keeps numeric fallback");
    }

    #[test]
    fn explicit_named_lines_place_a_span() {
        let parent = Style::default()
            .grid_cols([crate::style::Track::Px(100.0); 3])
            .grid_col_names(["sidebar", "divider", "content", "edge"]);
        let grid = resolve(&parent).expect("has line names");
        let child = Style::default().grid_col_lines("sidebar", "content");
        let (col, _row) = place(&child, &grid);
        // line "sidebar" = 1, line "content" = 3 → start 1, span 2.
        assert_eq!(col.start, Some(1));
        assert_eq!(col.span, Some(2));
    }

    #[test]
    fn no_names_means_no_resolved_grid() {
        assert!(resolve(&Style::default().grid_cols([crate::style::Track::Fr(1.0)])).is_none());
    }
}
