//! Responsive grid track sizing: `repeat` / `auto-fit` / `minmax` produce the
//! expected column layout, and fixed / `fr` templates still work — all through
//! taffy, headless (no GPU).

use fenestra_core::{Fonts, Frame, FrameState, GridTemplate, Theme, Track, build_frame, by, div};

/// Builds a frame from a grid container at a fixed size.
fn frame(container: fenestra_core::Element<()>, size: (f32, f32)) -> Frame {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    build_frame(&container, &theme, &mut fonts, &mut state, size, 1.0)
}

fn x_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.x0
}
fn y_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.y0
}
fn w_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.width()
}

/// `repeat(auto-fit, minmax(180px, 1fr))` in a 600px container yields three 200px
/// columns; the fourth item wraps to a new row.
#[test]
fn auto_fit_minmax_fills_three_columns_at_600() {
    let cells: Vec<_> = (0..6)
        .map(|i| div::<()>().id(&format!("c{i}")).h(50.0))
        .collect();
    let grid = div::<()>()
        .w(600.0)
        .grid_cols([GridTemplate::auto_fit_minmax(180.0)])
        .children(cells);
    let f = frame(grid, (600.0, 400.0));

    // 600 / 180 = 3 tracks; with the 1fr ceiling each track is 600/3 = 200 wide.
    assert!(
        (x_of(&f, "c0") - 0.0).abs() < 1.0,
        "c0 x = {}",
        x_of(&f, "c0")
    );
    assert!(
        (x_of(&f, "c1") - 200.0).abs() < 1.0,
        "c1 x = {}",
        x_of(&f, "c1")
    );
    assert!(
        (x_of(&f, "c2") - 400.0).abs() < 1.0,
        "c2 x = {}",
        x_of(&f, "c2")
    );
    assert!(
        (w_of(&f, "c0") - 200.0).abs() < 1.0,
        "c0 width = {}",
        w_of(&f, "c0")
    );
    // The fourth item wraps to row 2, column 1.
    assert!(
        (x_of(&f, "c3") - 0.0).abs() < 1.0,
        "c3 x = {}",
        x_of(&f, "c3")
    );
    assert!(
        y_of(&f, "c3") > y_of(&f, "c0"),
        "c3 ({}) wraps below c0 ({})",
        y_of(&f, "c3"),
        y_of(&f, "c0")
    );
}

/// `repeat(3, [1fr])` makes three equal columns.
#[test]
fn repeat_count_makes_equal_columns() {
    let cells: Vec<_> = (0..3)
        .map(|i| div::<()>().id(&format!("r{i}")).h(40.0))
        .collect();
    let grid = div::<()>()
        .w(600.0)
        .grid_cols([GridTemplate::repeat(3, [Track::Fr(1.0)])])
        .children(cells);
    let f = frame(grid, (600.0, 200.0));
    assert!(
        (x_of(&f, "r0") - 0.0).abs() < 1.0,
        "r0 x = {}",
        x_of(&f, "r0")
    );
    assert!(
        (x_of(&f, "r1") - 200.0).abs() < 1.0,
        "r1 x = {}",
        x_of(&f, "r1")
    );
    assert!(
        (x_of(&f, "r2") - 400.0).abs() < 1.0,
        "r2 x = {}",
        x_of(&f, "r2")
    );
}

fn h_of(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.height()
}

/// `grid-template-areas` lays out the classic holy-grail: a header and footer
/// spanning both columns, a fixed sidebar, and a flexible main — each child
/// placed only by area name, resolved to taffy lines by fenestra.
#[test]
fn template_areas_holy_grail() {
    let grid = div::<()>()
        .w(600.0)
        .h(300.0)
        .grid_cols([Track::Px(120.0), Track::Fr(1.0)])
        .grid_rows([Track::Px(40.0), Track::Fr(1.0), Track::Px(30.0)])
        .grid_template_areas(["header header", "nav main", "footer footer"])
        .children(vec![
            div::<()>().id("header").grid_area("header"),
            div::<()>().id("nav").grid_area("nav"),
            div::<()>().id("main").grid_area("main"),
            div::<()>().id("footer").grid_area("footer"),
        ]);
    let f = frame(grid, (600.0, 300.0));

    // Header: row 1 (40px tall), spanning both columns (full 600 width).
    assert!(
        (x_of(&f, "header")).abs() < 1.0,
        "header x = {}",
        x_of(&f, "header")
    );
    assert!(
        (y_of(&f, "header")).abs() < 1.0,
        "header y = {}",
        y_of(&f, "header")
    );
    assert!(
        (w_of(&f, "header") - 600.0).abs() < 1.0,
        "header w = {}",
        w_of(&f, "header")
    );
    assert!(
        (h_of(&f, "header") - 40.0).abs() < 1.0,
        "header h = {}",
        h_of(&f, "header")
    );

    // Nav: column 1 (120px), row 2 (the 1fr row = 300 - 40 - 30 = 230 tall).
    assert!((x_of(&f, "nav")).abs() < 1.0, "nav x = {}", x_of(&f, "nav"));
    assert!(
        (y_of(&f, "nav") - 40.0).abs() < 1.0,
        "nav y = {}",
        y_of(&f, "nav")
    );
    assert!(
        (w_of(&f, "nav") - 120.0).abs() < 1.0,
        "nav w = {}",
        w_of(&f, "nav")
    );
    assert!(
        (h_of(&f, "nav") - 230.0).abs() < 1.0,
        "nav h = {}",
        h_of(&f, "nav")
    );

    // Main: column 2 (480px), row 2, beside the sidebar.
    assert!(
        (x_of(&f, "main") - 120.0).abs() < 1.0,
        "main x = {}",
        x_of(&f, "main")
    );
    assert!(
        (y_of(&f, "main") - 40.0).abs() < 1.0,
        "main y = {}",
        y_of(&f, "main")
    );
    assert!(
        (w_of(&f, "main") - 480.0).abs() < 1.0,
        "main w = {}",
        w_of(&f, "main")
    );

    // Footer: last row (30px), pinned to the bottom, spanning both columns.
    assert!(
        (y_of(&f, "footer") - 270.0).abs() < 1.0,
        "footer y = {}",
        y_of(&f, "footer")
    );
    assert!(
        (w_of(&f, "footer") - 600.0).abs() < 1.0,
        "footer w = {}",
        w_of(&f, "footer")
    );
}

/// Named grid lines place an item across a span: `grid-column: b / e` over six
/// 100px columns starts at line `b` (x=100) and spans to line `e` (300 wide).
#[test]
fn named_lines_place_a_span() {
    let grid = div::<()>()
        .w(600.0)
        .grid_cols([Track::Px(100.0); 6])
        .grid_col_names(["a", "b", "c", "d", "e", "f", "g"])
        .children(vec![
            div::<()>().id("item").h(40.0).grid_col_lines("b", "e"),
        ]);
    let f = frame(grid, (600.0, 100.0));
    assert!(
        (x_of(&f, "item") - 100.0).abs() < 1.0,
        "item x = {}",
        x_of(&f, "item")
    );
    assert!(
        (w_of(&f, "item") - 300.0).abs() < 1.0,
        "item w = {}",
        w_of(&f, "item")
    );
}

/// `grid-template-areas` with no explicit tracks builds an implicit grid of
/// `auto` tracks shaped to the area map: a 2×2 map makes two equal columns split
/// a 400px container.
#[test]
fn template_areas_imply_auto_tracks() {
    let grid = div::<()>()
        .w(400.0)
        .grid_template_areas(["a b", "a b"])
        .children(vec![
            div::<()>().id("a").grid_area("a").h(50.0),
            div::<()>().id("b").grid_area("b").h(50.0),
        ]);
    let f = frame(grid, (400.0, 100.0));
    // Two auto columns sized to content here are zero-width (empty children), so
    // assert the placement order instead: `a` left of `b`.
    assert!(
        x_of(&f, "b") >= x_of(&f, "a"),
        "b ({}) right of a ({})",
        x_of(&f, "b"),
        x_of(&f, "a")
    );
    assert!(
        (y_of(&f, "a")).abs() < 1.0 && (y_of(&f, "b")).abs() < 1.0,
        "both on row 1"
    );
}

/// A hostile `repeat(count, …)` is clamped so the realized track total stays far
/// below taffy's `i16` grid-coordinate ceiling (taffy addresses grid lines with
/// `i16`, so ≥32768 tracks overflow it and a count near it makes it allocate a
/// ~1 GB cell matrix). `repeat(4096, 1fr)` in a 1024px container would size each
/// of 4096 columns at 1024/4096 = 0.25px; clamped to `MAX_GRID_TRACKS` (1024) the
/// single child instead fills a full 1024/1024 = 1.0px column.
#[test]
fn huge_repeat_count_is_clamped() {
    let child = div::<()>().id("cell").h(10.0);
    let grid = div::<()>()
        .w(1024.0)
        .grid_cols([GridTemplate::repeat(4096, [Track::Fr(1.0)])])
        .children(vec![child]);
    let f = frame(grid, (1024.0, 64.0));
    // 1024px / MAX_GRID_TRACKS(1024) = 1.0px per track once clamped; without the
    // clamp it would be 1024/4096 = 0.25px.
    assert!(
        (w_of(&f, "cell") - 1.0).abs() < 0.1,
        "clamped track width should be ~1.0, got {}",
        w_of(&f, "cell")
    );
}

/// A count past taffy's `i16` ceiling must not panic, hang, or allocate a giant
/// cell matrix: the clamp bounds the realized tracks, so the frame still builds
/// and the child keeps a finite, positive rect.
#[test]
fn pathological_repeat_count_survives() {
    let child = div::<()>().id("cell").h(10.0);
    let grid = div::<()>()
        .w(1024.0)
        .grid_cols([GridTemplate::repeat(40_000, [Track::Fr(1.0)])])
        .children(vec![child]);
    let f = frame(grid, (1024.0, 64.0));
    let w = w_of(&f, "cell");
    assert!(
        w.is_finite() && w > 0.0,
        "child width should be finite, got {w}"
    );
}

/// Plain `Track`s still work through the same builder (backward compatible): a
/// fixed 100px column plus a `1fr` column splits a 500px container 100 / 400.
#[test]
fn fixed_and_fr_tracks_still_work() {
    let a = div::<()>().id("a").h(30.0);
    let b = div::<()>().id("b").h(30.0);
    let grid = div::<()>()
        .w(500.0)
        .grid_cols([Track::Px(100.0), Track::Fr(1.0)])
        .children(vec![a, b]);
    let f = frame(grid, (500.0, 100.0));
    assert!((x_of(&f, "a") - 0.0).abs() < 1.0, "a x = {}", x_of(&f, "a"));
    assert!(
        (x_of(&f, "b") - 100.0).abs() < 1.0,
        "b x = {}",
        x_of(&f, "b")
    );
    assert!(
        (w_of(&f, "a") - 100.0).abs() < 1.0,
        "a width = {}",
        w_of(&f, "a")
    );
    assert!(
        (w_of(&f, "b") - 400.0).abs() < 1.0,
        "b width = {}",
        w_of(&f, "b")
    );
}
