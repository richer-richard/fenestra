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

use fenestra_core::{
    Color, Element, Mode, Semantics, TextSize, Theme, col, div, oklch, oklch_of, path, row, stack,
    text,
};
use kurbo::BezPath;

/// Observable10 — Observable Plot's default categorical scale. Charts are the
/// recognized exception to "color only through theme tokens": a plot with many
/// series needs many distinguishable hues, so they come from one principled,
/// mode-aware set rather than ad-hoc picks.
const OBSERVABLE10: [u32; 10] = [
    0x4269d0, 0xefb118, 0xff725c, 0x6cc5b0, 0x3ca951, 0xff8ab7, 0xa463f2, 0x97bbf5, 0x9c6b4e,
    0x9498a0,
];

fn rgb24(hex: u32) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xff) as u8,
        ((hex >> 8) & 0xff) as u8,
        (hex & 0xff) as u8,
        255,
    )
}

#[expect(clippy::cast_precision_loss, reason = "palette index counts are tiny")]
fn ramp_t(i: usize, n: usize) -> f32 {
    if n <= 1 {
        0.0
    } else {
        i as f32 / (n - 1) as f32
    }
}

/// Categorical, sequential, and diverging chart palettes — mode-aware and
/// generated in OKLCH (via [`fenestra_core::oklch`]) so every swatch is in
/// gamut. Categorical is [`Observable10`](https://observablehq.com/plot);
/// sequential and diverging are generated from hue inputs.
pub struct ChartPalette;

impl ChartPalette {
    /// The 10 categorical series colors for `mode`. Light is Observable10
    /// verbatim; dark is *re-picked* — each swatch lifted in lightness and
    /// eased in chroma so it reads on a dark canvas — never naively inverted.
    #[must_use]
    pub fn categorical(mode: Mode) -> [Color; 10] {
        std::array::from_fn(|i| {
            let base = rgb24(OBSERVABLE10[i]);
            match mode {
                Mode::Light => base,
                Mode::Dark => {
                    let [l, c, h] = oklch_of(base);
                    oklch((l + 0.08).min(0.95), c * 0.82, h)
                }
            }
        })
    }

    /// The categorical color for series `i` (wraps after 10).
    #[must_use]
    pub fn series(i: usize, mode: Mode) -> Color {
        Self::categorical(mode)[i % 10]
    }

    /// A single-hue sequential ramp of `n` swatches: lightness ramps linearly
    /// from pale to deep while chroma rises toward the saturated end — the
    /// standard OKLCH sequential recipe. Dark mode keeps the ramp brighter.
    #[must_use]
    pub fn sequential(hue: f32, n: usize, mode: Mode) -> Vec<Color> {
        let (l0, l1) = match mode {
            Mode::Light => (0.95, 0.40),
            Mode::Dark => (0.90, 0.45),
        };
        (0..n)
            .map(|i| {
                let t = ramp_t(i, n);
                oklch(l0 + (l1 - l0) * t, 0.04 + (0.16 - 0.04) * t, hue)
            })
            .collect()
    }

    /// A diverging ramp of `n` swatches through a light neutral midpoint: two
    /// single-hue arms (`hue_low` ↔ `hue_high`) ramping from deep, saturated
    /// ends to a near-white center. An odd `n` keeps the neutral dead-center.
    #[must_use]
    pub fn diverging(hue_low: f32, hue_high: f32, n: usize, mode: Mode) -> Vec<Color> {
        if n == 0 {
            return Vec::new();
        }
        let l_center = match mode {
            Mode::Light => 0.95,
            Mode::Dark => 0.90,
        };
        #[expect(clippy::cast_precision_loss, reason = "swatch counts are tiny")]
        let mid = (n - 1) as f32 / 2.0;
        (0..n)
            .map(|i| {
                #[expect(clippy::cast_precision_loss, reason = "swatch counts are tiny")]
                let d = (i as f32 - mid).abs() / mid.max(1.0);
                let hue = if (i as f32) < mid { hue_low } else { hue_high };
                oklch(l_center - 0.50 * d, 0.02 + 0.16 * d, hue)
            })
            .collect()
    }
}

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

/// A multi-series line chart: every series shares one normalized 0..1 range
/// (so the lines are comparable on one axis) and is stroked in its
/// [`ChartPalette`] categorical color, adapted to the theme's mode. Defaults
/// 320x160; size with `.w/.h`.
pub fn multi_line_chart<Msg>(
    series: impl IntoIterator<Item = (impl Into<String>, Vec<f32>)>,
) -> Element<Msg> {
    let series: Vec<(String, Vec<f32>)> = series
        .into_iter()
        .map(|(label, v)| (label.into(), v))
        .collect();
    // One shared min/max across every finite value, so series are comparable.
    let (min, max) = series
        .iter()
        .flat_map(|(_, v)| v.iter().copied())
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    let range = max - min;
    let mut panel = stack()
        .w(320.0)
        .h(160.0)
        .p(8.0)
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
        .semantics(Semantics::Image)
        .label("multi-series line chart");
    for (i, (_label, values)) in series.into_iter().enumerate() {
        let points: Vec<f32> = values
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .map(|v| {
                if range > f32::EPSILON {
                    (v - min) / range
                } else {
                    0.5
                }
            })
            .collect();
        if points.len() < 2 {
            continue;
        }
        #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
        let viewbox = ((points.len() - 1) as f64, 1.0);
        panel = panel.child(
            path(polyline(&points), viewbox, Some(0.025))
                .w_full()
                .h_full()
                .themed(move |t: &Theme, s| s.color(ChartPalette::series(i, t.mode))),
        );
    }
    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use fenestra_core::oklch_of;

    #[test]
    fn observable10_light_is_verbatim() {
        let p = ChartPalette::categorical(Mode::Light);
        assert_eq!(p.len(), 10);
        assert_eq!(p[0].to_rgba8(), rgb24(0x4269d0).to_rgba8());
        assert_eq!(p[9].to_rgba8(), rgb24(0x9498a0).to_rgba8());
    }

    #[test]
    fn dark_is_repicked_lighter_not_inverted() {
        let light = ChartPalette::categorical(Mode::Light);
        let dark = ChartPalette::categorical(Mode::Dark);
        for i in 0..10 {
            let ll = oklch_of(light[i])[0];
            let dl = oklch_of(dark[i])[0];
            assert!(
                dl + 1e-3 >= ll,
                "dark swatch {i} must be >= light lightness (re-pick, not invert): {dl} vs {ll}"
            );
        }
        assert_ne!(
            light[0].to_rgba8(),
            dark[0].to_rgba8(),
            "dark must actually differ from light"
        );
    }

    #[test]
    fn sequential_ramps_light_to_dark_with_count() {
        let s = ChartPalette::sequential(260.0, 5, Mode::Light);
        assert_eq!(s.len(), 5);
        let l: Vec<f32> = s.iter().map(|c| oklch_of(*c)[0]).collect();
        assert!(l[0] > l[4], "sequential must darken across the ramp: {l:?}");
    }

    #[test]
    fn diverging_center_is_near_neutral_and_light() {
        let d = ChartPalette::diverging(260.0, 30.0, 5, Mode::Light);
        assert_eq!(d.len(), 5);
        let center = oklch_of(d[2]);
        let end = oklch_of(d[0]);
        assert!(
            center[1] < end[1],
            "diverging center must be less saturated than the ends"
        );
        assert!(
            center[0] > end[0],
            "diverging center must be lighter than the ends"
        );
    }
}
