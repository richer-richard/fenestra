//! Pixel-locked chart rendering, plus the no-panic contract on hostile
//! data — the bar a widget crate must clear.

use fenestra_charts::{bar_chart, line_chart, sparkline};
use fenestra_core::{Element, Theme, col, row, text};
use fenestra_shell::render_element;
use fenestra_shell::testing::assert_png_snapshot;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

#[test]
fn charts_render_to_golden() {
    let view: Element<()> = col().p(16.0).gap(12.0).children((
        row().gap(8.0).items_center().children((
            text("requests/min"),
            sparkline([3.0, 5.0, 2.0, 8.0, 6.0, 9.0, 4.0, 7.0]),
        )),
        line_chart([12.0, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0, 29.0]),
        bar_chart([
            ("mon", 4.0),
            ("tue", 7.0),
            ("wed", 3.0),
            ("thu", 9.0),
            ("fri", 6.0),
        ]),
    ));
    let image = render_element(view, &Theme::light(), (380, 420));
    assert_png_snapshot(snapshot_dir(), "charts", &image);
}

#[test]
fn hostile_data_never_panics() {
    let view: Element<()> = col().p(8.0).gap(8.0).children((
        sparkline(std::iter::empty::<f32>()),
        sparkline([f32::NAN, f32::INFINITY, 1.0]),
        sparkline([5.0]),
        sparkline([2.0, 2.0, 2.0]), // flat
        line_chart([f32::NAN, f32::NEG_INFINITY]),
        bar_chart([("neg", -3.0), ("nan", f32::NAN), ("ok", 1.0)]),
        bar_chart(Vec::<(String, f32)>::new()),
    ));
    let image = render_element(view, &Theme::dark(), (380, 300));
    assert_eq!(image.dimensions(), (380, 300));
}
