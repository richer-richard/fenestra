//! Charts for fenestra: sparklines, line charts, and bar charts — and
//! the reference for writing a fenestra widget crate. Everything here
//! uses only `fenestra-core`'s public API: plain functions returning
//! [`Element`]s, colors through theme tokens, stable semantics, no
//! macros, no panics on hostile data (non-finite values are skipped,
//! empty input renders an empty element).
//!
//! ```
//! use fenestra_charts::{bar_chart, line_chart, sparkline};
//!
//! let el: fenestra_core::Element<()> = sparkline([3.0, 1.0, 4.0, 1.0, 5.0]);
//! let chart: fenestra_core::Element<()> = line_chart([3.0, 1.0, 4.0, 1.0, 5.0]).h(160.0);
//! let bars: fenestra_core::Element<()> = bar_chart([("a", 3.0), ("b", 7.0), ("c", 5.0)]);
//! ```

use fenestra_core::{Element, Semantics, TextSize, Theme, col, div, path, row, text};
use kurbo::BezPath;

/// Normalizes finite values into 0..=1 (min -> 0, max -> 1). A flat
/// series maps to 0.5.
fn normalized(values: &[f32]) -> Vec<f32> {
    let finite: Vec<f32> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let (min, max) = finite
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        });
    let range = max - min;
    finite
        .iter()
        .map(|v| {
            if range > f32::EPSILON {
                (v - min) / range
            } else {
                0.5
            }
        })
        .collect()
}

/// The polyline through normalized points in a `(len-1) x 1` viewbox,
/// y flipped (larger values up).
fn polyline(points: &[f32]) -> BezPath {
    let mut bez = BezPath::new();
    for (i, v) in points.iter().enumerate() {
        let x = i as f64;
        let y = f64::from(1.0 - v);
        if i == 0 {
            bez.move_to((x, y));
        } else {
            bez.line_to((x, y));
        }
    }
    bez
}

/// A tiny inline trend line (defaults 96x24; size with `.w/.h`).
/// Stroked in the theme accent.
pub fn sparkline<Msg>(values: impl IntoIterator<Item = f32>) -> Element<Msg> {
    let values: Vec<f32> = values.into_iter().collect();
    let points = normalized(&values);
    if points.len() < 2 {
        return div().w(96.0).h(24.0);
    }
    #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
    let viewbox = ((points.len() - 1) as f64, 1.0);
    path(polyline(&points), viewbox, Some(0.06))
        .w(96.0)
        .h(24.0)
        .themed(|t: &Theme, s| s.color(t.accent))
        .semantics(Semantics::Image)
        .label("sparkline")
}

/// A line chart panel: the series stroked in the accent over a subtle
/// baseline grid. Defaults 320x160; size with `.w/.h`.
pub fn line_chart<Msg>(values: impl IntoIterator<Item = f32>) -> Element<Msg> {
    let values: Vec<f32> = values.into_iter().collect();
    let points = normalized(&values);
    let mut panel = col()
        .w(320.0)
        .h(160.0)
        .p(8.0)
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
        .semantics(Semantics::Image)
        .label("line chart");
    if points.len() >= 2 {
        #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
        let viewbox = ((points.len() - 1) as f64, 1.0);
        panel = panel.child(
            path(polyline(&points), viewbox, Some(0.025))
                .w_full()
                .h_full()
                .themed(|t: &Theme, s| s.color(t.accent)),
        );
    }
    panel
}

/// A labeled bar chart: flexbox bars filled with the accent, value-
/// proportional heights, labels underneath. Defaults 320x160.
pub fn bar_chart<Msg>(bars: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Element<Msg> {
    let bars: Vec<(String, f32)> = bars
        .into_iter()
        .map(|(label, v)| (label.into(), v))
        .filter(|(_, v)| v.is_finite() && *v >= 0.0)
        .collect();
    let max = bars.iter().map(|(_, v)| *v).fold(0.0_f32, f32::max);
    row()
        .w(320.0)
        .h(160.0)
        .p(8.0)
        .gap(8.0)
        .items_end()
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
        .semantics(Semantics::Image)
        .label("bar chart")
        .children(bars.into_iter().map(move |(label, v)| {
            let fraction = if max > f32::EPSILON { v / max } else { 0.0 };
            col().grow().h_full().gap(4.0).justify_end().children((
                div()
                    .w_full()
                    .h(fenestra_core::Length::Pct(fraction * 88.0))
                    .rounded(3.0)
                    .themed(|t: &Theme, s| s.bg(t.accent)),
                text(label)
                    .size(TextSize::Xs)
                    .tabular()
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            ))
        }))
}
