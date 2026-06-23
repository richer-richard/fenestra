//! Headless state tests for the matured `data_table`: virtualization only
//! materializes the visible window, explicit widths become fixed pixel
//! columns, a display order permutes the columns while cells keep tracking
//! their data column, and the filter row surfaces the app's filter text. No
//! GPU — just `build_frame` and semantic queries.

use fenestra_core::{Element, Fonts, Frame, FrameState, Theme, build_frame, by, col};
use fenestra_kit::data_table;

fn build(view: &Element<()>, size: (f32, f32)) -> Frame {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    build_frame(view, &theme, &mut fonts, &mut state, size, 1.0)
}

fn x_of(f: &Frame, label: &str) -> f64 {
    f.get(&by::label(label)).rect.x0
}

/// A wide table (200 rows) inside a short, bounded viewport realizes only the
/// rows near the top — the far end never materializes.
#[test]
fn virtualization_realizes_only_the_visible_window() {
    let rows: Vec<Vec<String>> = (0..200)
        .map(|i| vec![format!("r{i}"), format!("v{i}")])
        .collect();
    let view: Element<()> = col().w(400.0).h(200.0).child(
        data_table(["Key", "Val"], rows)
            .id("big")
            .sticky_header(true),
    );
    let f = build(&view, (400.0, 220.0));

    assert!(
        f.query(&by::label("r0")).is_some(),
        "the first row is realized"
    );
    assert!(
        f.query(&by::label("r199")).is_none(),
        "the far end is never built"
    );
    // The body is a real scroll container, keyed for offset persistence.
    assert!(
        f.query(&by::id("dt-body-big")).is_some(),
        "the virtualized body keeps its stable id"
    );
}

/// Explicit `column_widths` lay the columns out at fixed pixel widths: adjacent
/// cells are exactly one column-width apart (Fr would stretch to the viewport).
#[test]
fn explicit_widths_produce_fixed_pixel_columns() {
    let view: Element<()> = col().w(700.0).h(200.0).child(
        data_table(
            ["Name", "Role", "Commits"],
            vec![vec!["Ripley".into(), "Officer".into(), "128".into()]],
        )
        .column_widths([160.0, 200.0, 100.0]),
    );
    let f = build(&view, (760.0, 240.0));

    // Column 1 starts one 160px column to the right of column 0; column 2 a
    // further 200px on. The deltas are the declared widths, not Fr shares.
    let (c0, c1, c2) = (x_of(&f, "Ripley"), x_of(&f, "Officer"), x_of(&f, "128"));
    assert!(
        (c1 - c0 - 160.0).abs() < 1.0,
        "column 0 is 160px wide: gap was {}",
        c1 - c0
    );
    assert!(
        (c2 - c1 - 200.0).abs() < 1.0,
        "column 1 is 200px wide: gap was {}",
        c2 - c1
    );
}

/// A reversed display order permutes the columns left-to-right while each cell
/// still reads from its own data column.
#[test]
fn column_order_permutes_columns_but_cells_track_data() {
    let view: Element<()> = col().w(700.0).h(200.0).child(
        data_table(
            ["Name", "Role", "Commits"],
            vec![vec!["Ripley".into(), "Officer".into(), "128".into()]],
        )
        .column_widths([160.0, 200.0, 100.0])
        .column_order([2, 1, 0]),
    );
    let f = build(&view, (760.0, 240.0));

    // Header labels appear in display order: Commits, Role, Name.
    assert!(
        x_of(&f, "Commits") < x_of(&f, "Role") && x_of(&f, "Role") < x_of(&f, "Name"),
        "headers render in the reversed display order"
    );
    // The body cells follow the same order, so the Commits value (data col 2)
    // now sits left of the Name value (data col 0).
    assert!(
        x_of(&f, "128") < x_of(&f, "Officer") && x_of(&f, "Officer") < x_of(&f, "Ripley"),
        "cells reorder with their data column"
    );
}

/// The filter row renders one input per column, each showing the app's current
/// filter text (the widget never filters the rows itself).
#[test]
fn filter_row_shows_per_column_text() {
    let view: Element<()> = col().w(700.0).h(200.0).child(
        data_table(
            ["Name", "Role", "Commits"],
            vec![vec!["Ripley".into(), "Officer".into(), "128".into()]],
        )
        .id("crew")
        .filter(["rip".into(), String::new(), String::new()])
        .on_filter(|_, _| ()),
    );
    let f = build(&view, (760.0, 240.0));

    assert!(
        f.query(&by::value("rip")).is_some(),
        "the first column's filter input shows the app text"
    );
    // Two empty filter inputs plus the prefilled one: three editors in all.
    // (Role queries ignore the payload, so `multiline` is immaterial here.)
    assert_eq!(
        f.get_all(&by::role(fenestra_core::Semantics::TextInput {
            multiline: false
        }))
        .len(),
        3,
        "one filter input per column"
    );
}
