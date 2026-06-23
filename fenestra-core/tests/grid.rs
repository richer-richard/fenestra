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
