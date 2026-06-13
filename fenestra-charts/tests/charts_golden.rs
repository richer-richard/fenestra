//! Pixel-locked chart rendering, plus the no-panic contract on hostile
//! data — the bar a widget crate must clear.

use fenestra_charts::{bar_chart, line_chart, multi_line_chart, sparkline};
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

/// The multi-series chart, locked in both modes — the dark palette is
/// re-picked (not inverted), so it earns its own golden.
#[test]
fn multi_line_chart_golden_light_and_dark() {
    let series = || {
        vec![
            (
                "cpu".to_string(),
                vec![12.0, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0],
            ),
            (
                "mem".to_string(),
                vec![20.0, 16.0, 22.0, 19.0, 28.0, 24.0, 30.0, 27.0],
            ),
            (
                "io".to_string(),
                vec![5.0, 9.0, 7.0, 12.0, 8.0, 14.0, 10.0, 16.0],
            ),
            (
                "net".to_string(),
                vec![14.0, 11.0, 17.0, 13.0, 9.0, 20.0, 15.0, 12.0],
            ),
        ]
    };
    let light: Element<()> = col().p(16.0).child(multi_line_chart(series()));
    assert_png_snapshot(
        snapshot_dir(),
        "multi_line_light",
        &render_element(light, &Theme::light(), (360, 200)),
    );
    let dark: Element<()> = col().p(16.0).child(multi_line_chart(series()));
    assert_png_snapshot(
        snapshot_dir(),
        "multi_line_dark",
        &render_element(dark, &Theme::dark(), (360, 200)),
    );
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
        multi_line_chart(vec![
            ("nan".to_string(), vec![f32::NAN, f32::INFINITY]),
            ("empty".to_string(), Vec::new()),
            ("one".to_string(), vec![3.0]),
        ]),
        multi_line_chart(Vec::<(String, Vec<f32>)>::new()),
    ));
    let image = render_element(view, &Theme::dark(), (380, 320));
    assert_eq!(image.dimensions(), (380, 320));
}
