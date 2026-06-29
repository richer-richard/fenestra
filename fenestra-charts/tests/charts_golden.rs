//! Pixel-locked chart rendering, plus the no-panic contract on hostile
//! data — the bar a widget crate must clear.

use fenestra_charts::{
    BarChartAxes, GroupedBarChart, LineChartBuilder, MultiSeriesChart, PieChart, ScatterChart,
    StackedBarChart, area_chart, bar_chart, line_chart, multi_line_chart, sparkline,
};
use fenestra_core::{Element, Theme, col, row, text};
use fenestra_shell::render_element;
use fenestra_shell::testing::assert_png_snapshot;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

// ── Original golden tests (unchanged) ────────────────────────────────────────

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

// ── New golden tests ──────────────────────────────────────────────────────────

/// Line chart with axes: y-axis ticks, gridlines, tick labels, x-labels,
/// and data-point markers. Locked in both light and dark.
#[test]
fn line_chart_axes_golden() {
    let values = [12.0_f32, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0, 29.0];
    let labels = [
        "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun", "Mon", "Tue",
    ];

    let light: Element<()> = col().p(16.0).child(
        LineChartBuilder::new(values)
            .x_labels(labels)
            .show_markers()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "line_chart_axes_light",
        &render_element(light, &Theme::light(), (360, 220)),
    );

    let dark: Element<()> = col().p(16.0).child(
        LineChartBuilder::new(values)
            .x_labels(labels)
            .show_markers()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "line_chart_axes_dark",
        &render_element(dark, &Theme::dark(), (360, 220)),
    );
}

/// Line chart with opt-in axis titles: the x-title sits below the tick labels
/// and the y-title is rotated along the left edge. Exercises the new titled
/// layout (untitled charts keep their original goldens). Light and dark.
#[test]
fn line_chart_titled_golden() {
    let values = [12.0_f32, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0, 29.0];
    let labels = [
        "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun", "Mon", "Tue",
    ];

    let light: Element<()> = col().p(16.0).child(
        LineChartBuilder::new(values)
            .x_labels(labels)
            .x_title("Day of week")
            .y_title("Requests/min")
            .show_markers()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "line_chart_titled_light",
        &render_element(light, &Theme::light(), (360, 240)),
    );

    let dark: Element<()> = col().p(16.0).child(
        LineChartBuilder::new(values)
            .x_labels(labels)
            .x_title("Day of week")
            .y_title("Requests/min")
            .show_markers()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "line_chart_titled_dark",
        &render_element(dark, &Theme::dark(), (360, 240)),
    );
}

/// Bar chart with y-axis, gridlines, and numeric value labels above bars.
/// Locked in both modes.
#[test]
fn bar_chart_axes_golden() {
    let data = [
        ("Mon", 4.0_f32),
        ("Tue", 7.0),
        ("Wed", 3.0),
        ("Thu", 9.0),
        ("Fri", 6.0),
    ];

    let light: Element<()> = col()
        .p(16.0)
        .child(BarChartAxes::new(data).show_values().build());
    assert_png_snapshot(
        snapshot_dir(),
        "bar_chart_axes_light",
        &render_element(light, &Theme::light(), (360, 220)),
    );

    let dark: Element<()> = col()
        .p(16.0)
        .child(BarChartAxes::new(data).show_values().build());
    assert_png_snapshot(
        snapshot_dir(),
        "bar_chart_axes_dark",
        &render_element(dark, &Theme::dark(), (360, 220)),
    );
}

/// Multi-series chart with axes and legend, markers visible in dark mode.
#[test]
fn multi_series_chart_golden() {
    let series = || {
        [
            (
                "cpu",
                vec![12.0_f32, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0],
            ),
            (
                "mem",
                vec![20.0_f32, 16.0, 22.0, 19.0, 28.0, 24.0, 30.0, 27.0],
            ),
            ("io", vec![5.0_f32, 9.0, 7.0, 12.0, 8.0, 14.0, 10.0, 16.0]),
        ]
    };

    let light: Element<()> = col().p(16.0).child(MultiSeriesChart::new(series()).build());
    assert_png_snapshot(
        snapshot_dir(),
        "multi_series_light",
        &render_element(light, &Theme::light(), (360, 260)),
    );

    let dark: Element<()> = col()
        .p(16.0)
        .child(MultiSeriesChart::new(series()).show_markers().build());
    assert_png_snapshot(
        snapshot_dir(),
        "multi_series_dark",
        &render_element(dark, &Theme::dark(), (360, 260)),
    );
}

/// Area chart: filled region + axis ticks. Light and dark.
#[test]
fn area_chart_golden() {
    let values = [12.0_f32, 18.0, 9.0, 26.0, 22.0, 31.0, 17.0, 24.0, 29.0];

    let light: Element<()> = col().p(16.0).child(area_chart(values));
    assert_png_snapshot(
        snapshot_dir(),
        "area_chart_light",
        &render_element(light, &Theme::light(), (360, 210)),
    );

    let dark: Element<()> = col().p(16.0).child(area_chart(values));
    assert_png_snapshot(
        snapshot_dir(),
        "area_chart_dark",
        &render_element(dark, &Theme::dark(), (360, 210)),
    );
}

/// Scatter chart: x and y axes, dot at each point. Light and dark.
#[test]
fn scatter_chart_golden() {
    let pts = [
        (1.0_f32, 2.3_f32),
        (2.0, 4.1),
        (3.0, 3.2),
        (4.0, 5.8),
        (5.0, 4.5),
        (6.0, 6.9),
        (7.0, 5.2),
        (8.0, 7.1),
        (2.5, 1.8),
        (6.5, 8.3),
    ];

    let light: Element<()> = col().p(16.0).child(ScatterChart::new(pts).build());
    assert_png_snapshot(
        snapshot_dir(),
        "scatter_chart_light",
        &render_element(light, &Theme::light(), (360, 280)),
    );

    let dark: Element<()> = col().p(16.0).child(ScatterChart::new(pts).build());
    assert_png_snapshot(
        snapshot_dir(),
        "scatter_chart_dark",
        &render_element(dark, &Theme::dark(), (360, 280)),
    );
}

/// Pie chart: filled arc slices + legend. Light and dark.
#[test]
fn pie_chart_golden() {
    let segs = [
        ("JavaScript", 35.0_f32),
        ("Python", 28.0),
        ("Rust", 15.0),
        ("Go", 12.0),
        ("Other", 10.0),
    ];

    let light: Element<()> = col().p(16.0).child(PieChart::new(segs).build());
    assert_png_snapshot(
        snapshot_dir(),
        "pie_chart_light",
        &render_element(light, &Theme::light(), (260, 280)),
    );

    let dark: Element<()> = col().p(16.0).child(PieChart::new(segs).build());
    assert_png_snapshot(
        snapshot_dir(),
        "pie_chart_dark",
        &render_element(dark, &Theme::dark(), (260, 280)),
    );
}

/// Donut chart: ring slices + legend. Light and dark.
#[test]
fn donut_chart_golden() {
    let segs = [
        ("JavaScript", 35.0_f32),
        ("Python", 28.0),
        ("Rust", 15.0),
        ("Go", 12.0),
        ("Other", 10.0),
    ];

    let light: Element<()> = col().p(16.0).child(PieChart::new(segs).donut(0.55).build());
    assert_png_snapshot(
        snapshot_dir(),
        "donut_chart_light",
        &render_element(light, &Theme::light(), (260, 280)),
    );

    let dark: Element<()> = col().p(16.0).child(PieChart::new(segs).donut(0.55).build());
    assert_png_snapshot(
        snapshot_dir(),
        "donut_chart_dark",
        &render_element(dark, &Theme::dark(), (260, 280)),
    );
}

/// Stacked bar chart: multi-series stacked + y-axis + legend. Light and dark.
#[test]
fn stacked_bar_chart_golden() {
    let cats = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    let series = [
        ("web", vec![3.0_f32, 5.0, 4.0, 6.0, 5.5]),
        ("api", vec![2.0_f32, 3.0, 6.0, 4.0, 3.5]),
        ("db", vec![1.0_f32, 2.0, 1.5, 2.5, 2.0]),
    ];

    let light: Element<()> = col().p(16.0).child(
        StackedBarChart::new(cats, series.clone())
            .show_values()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "stacked_bar_light",
        &render_element(light, &Theme::light(), (360, 260)),
    );

    let dark: Element<()> = col()
        .p(16.0)
        .child(StackedBarChart::new(cats, series).build());
    assert_png_snapshot(
        snapshot_dir(),
        "stacked_bar_dark",
        &render_element(dark, &Theme::dark(), (360, 260)),
    );
}

/// Grouped bar chart: side-by-side series + y-axis + legend. Light and dark.
#[test]
fn grouped_bar_chart_golden() {
    let cats = ["Q1", "Q2", "Q3", "Q4"];
    let series = [
        ("product_a", vec![3.0_f32, 5.0, 4.0, 6.0]),
        ("product_b", vec![2.0_f32, 4.0, 3.0, 5.0]),
        ("product_c", vec![1.5_f32, 2.5, 2.0, 3.5]),
    ];

    let light: Element<()> = col().p(16.0).child(
        GroupedBarChart::new(cats, series.clone())
            .show_values()
            .build(),
    );
    assert_png_snapshot(
        snapshot_dir(),
        "grouped_bar_light",
        &render_element(light, &Theme::light(), (360, 260)),
    );

    let dark: Element<()> = col()
        .p(16.0)
        .child(GroupedBarChart::new(cats, series).build());
    assert_png_snapshot(
        snapshot_dir(),
        "grouped_bar_dark",
        &render_element(dark, &Theme::dark(), (360, 260)),
    );
}

/// Hostile data on every new chart type: must not panic and must produce
/// the correct canvas dimensions.
#[test]
fn hostile_data_new_charts_never_panics() {
    let view: Element<()> = col().p(8.0).gap(8.0).children((
        LineChartBuilder::new([f32::NAN, f32::INFINITY, -1.0, 0.0, 1.0])
            .show_markers()
            .build::<()>(),
        BarChartAxes::new([
            ("neg", -5.0_f32),
            ("nan", f32::NAN),
            ("inf", f32::INFINITY),
            ("ok", 3.0),
        ])
        .show_values()
        .build::<()>(),
        area_chart([f32::NAN, f32::INFINITY, 0.0]),
        ScatterChart::new([(f32::NAN, 1.0_f32), (1.0, f32::NAN), (2.0, 3.0)]).build::<()>(),
        PieChart::new([("zero", 0.0_f32), ("nan", f32::NAN), ("ok", 5.0)]).build::<()>(),
        PieChart::new([("ok", 1.0_f32)]).donut(0.55).build::<()>(),
        StackedBarChart::new(
            ["x", "y"],
            [
                ("s1", vec![f32::NAN, 2.0_f32]),
                ("s2", vec![1.0_f32, f32::INFINITY]),
            ],
        )
        .build::<()>(),
        GroupedBarChart::new(["x"], [("s", vec![f32::NAN, f32::INFINITY, -1.0_f32])]).build::<()>(),
    ));
    let image = render_element(view, &Theme::dark(), (380, 600));
    assert_eq!(image.dimensions(), (380, 600));
}
