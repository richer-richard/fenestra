//! Charts for fenestra: sparklines, line charts, bar charts, multi-series
//! line charts, area charts, scatter charts, pie / donut charts, stacked bar
//! charts, grouped bar charts — and the reference for writing a fenestra
//! widget crate.
//!
//! Everything here uses only `fenestra-core`'s public API: plain functions
//! or builder types returning [`Element`]s, colors through theme tokens and
//! [`ChartPalette`], stable semantics, no macros, no panics on hostile data
//! (non-finite values are skipped, empty input renders an empty element).
//!
//! ```
//! use fenestra_charts::{bar_chart, line_chart, sparkline, LineChartBuilder};
//!
//! let el: fenestra_core::Element<()> = sparkline([3.0, 1.0, 4.0, 1.0, 5.0]);
//! let chart: fenestra_core::Element<()> = line_chart([3.0, 1.0, 4.0, 1.0, 5.0]).h(160.0);
//! let bars: fenestra_core::Element<()> = bar_chart([("a", 3.0), ("b", 7.0), ("c", 5.0)]);
//! let full: fenestra_core::Element<()> =
//!     LineChartBuilder::new([3.0, 1.0, 4.0]).show_markers().build();
//! ```

use std::f64::consts::{FRAC_PI_2, TAU};

use fenestra_core::{
    Color, Element, Mode, Semantics, TextAlign, TextSize, Theme, col, div, oklch, oklch_of, path,
    row, stack, text,
};
use kurbo::{Arc, BezPath};

// ── Observable10 categorical palette ─────────────────────────────────────────

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

// ── Data normalization helpers ────────────────────────────────────────────────

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

/// Maps finite values into [0, 1] relative to an explicit `[lo, hi]` range.
/// Values outside the range are clamped. Non-finite values are excluded.
fn to_unit_range(values: &[f32], lo: f32, hi: f32) -> Vec<f32> {
    let range = (hi - lo).max(f32::EPSILON);
    values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| ((v - lo) / range).clamp(0.0, 1.0))
        .collect()
}

/// The polyline through normalized points in a `(len-1) x 1` viewbox,
/// y flipped (larger values up).
fn polyline(points: &[f32]) -> BezPath {
    let mut bez = BezPath::new();
    for (i, v) in points.iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
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

// ── Tick math ─────────────────────────────────────────────────────────────────

/// Round `raw` to the nearest 1/2/5 × 10^k (nice numbers algorithm).
/// Returns `1.0` for non-positive or non-finite inputs.
fn nice_step(raw: f32) -> f32 {
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let mag = raw.log10().floor();
    let pow = 10f32.powf(mag);
    let frac = raw / pow;
    if frac <= 1.0 {
        pow
    } else if frac <= 2.0 {
        2.0 * pow
    } else if frac <= 5.0 {
        5.0 * pow
    } else {
        10.0 * pow
    }
}

/// Generate nicely-spaced tick values covering `[lo, hi]` with approximately
/// `target` ticks (minimum 2). Non-finite bounds default to `[0, 1]`. The
/// returned slice is at most `target * 4 + 2` elements so hostile inputs are
/// safe.
fn nice_ticks(lo: f32, hi: f32, target: usize) -> Vec<f32> {
    let target = target.max(2);
    let lo = if lo.is_finite() { lo } else { 0.0 };
    let hi = if hi.is_finite() { hi } else { 1.0 };
    let (lo, hi) = if (hi - lo).abs() < f32::EPSILON {
        (lo - 1.0, lo + 1.0)
    } else if lo > hi {
        (hi, lo)
    } else {
        (lo, hi)
    };

    #[expect(clippy::cast_precision_loss, reason = "target tick counts are small")]
    let step = nice_step((hi - lo) / target as f32);
    if step <= 0.0 {
        return vec![lo, hi];
    }
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let max_count = target * 4 + 2;
    let mut ticks = Vec::with_capacity(target + 2);
    let mut v = start;
    while v <= end + step * 0.5 && ticks.len() < max_count {
        ticks.push(v);
        v += step;
    }
    ticks
}

/// Format a tick value: integer steps get zero decimals; sub-integer steps
/// use enough digits to distinguish adjacent ticks.
fn fmt_tick(v: f32, step: f32) -> String {
    if !v.is_finite() {
        return "–".to_string();
    }
    let decimals: usize = if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    };
    format!("{v:.decimals$}")
}

// ── Accessible descriptions ─────────────────────────────────────────────────
//
// The strategic point of these charts is that an agent can verify them without
// pixels: it renders headlessly and reads the typed access tree. A static
// `.label("line chart")` told it nothing it could check. Each chart instead
// projects a concise, data-derived description (counts, range, trend, slices)
// so the series count, value range, categories, and direction are all
// machine-checkable. These strings are accessible metadata only — never
// painted — so enriching them changes no pixels.

/// Compact, deterministic, locale-free rendering of a data value: at most
/// three decimals, with trailing zeros (and a dangling point) trimmed.
/// Non-finite values render as `n/a`.
fn fmt_num(v: f32) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    // Normalize -0.0 so it prints as "0", not "-0".
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// `"1 point"` / `"7 points"` — pluralize a count with its noun (naive `+s`).
/// Not for nouns whose plural is irregular (e.g. "series" is invariant).
fn count_noun(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// Finite minimum and maximum across `values`, or `None` when none are finite.
fn finite_min_max(values: &[f32]) -> Option<(f32, f32)> {
    let (lo, hi) = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    (lo.is_finite() && hi.is_finite()).then_some((lo, hi))
}

/// Overall direction of a series: compares the first and last finite values,
/// reporting near-level runs (within 1% of the data range) as `flat`.
fn trend_word(values: &[f32]) -> &'static str {
    let finite: Vec<f32> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let (Some(&first), Some(&last)) = (finite.first(), finite.last()) else {
        return "flat";
    };
    let Some((lo, hi)) = finite_min_max(&finite) else {
        return "flat";
    };
    let span = hi - lo;
    let delta = last - first;
    if span <= f32::EPSILON || delta.abs() <= span * 0.01 {
        "flat"
    } else if delta > 0.0 {
        "rising"
    } else {
        "falling"
    }
}

/// Join `labels` with commas, showing at most `max`; a longer list ends with
/// `"+K more"` so the description stays bounded on hostile input.
fn bounded_labels(labels: &[String], max: usize) -> String {
    if labels.len() <= max {
        labels.join(", ")
    } else {
        let shown = labels[..max].join(", ");
        format!("{shown}, +{} more", labels.len() - max)
    }
}

/// Accessible summary for a single-series line / area / sparkline chart:
/// `"<kind>, N points, min <lo>, max <hi>, <trend>"`.
fn describe_line(kind: &str, values: &[f32]) -> String {
    let finite: Vec<f32> = values.iter().copied().filter(|v| v.is_finite()).collect();
    match finite_min_max(&finite) {
        None => format!("{kind}, no data"),
        Some((lo, hi)) => format!(
            "{kind}, {}, min {}, max {}, {}",
            count_noun(finite.len(), "point"),
            fmt_num(lo),
            fmt_num(hi),
            trend_word(&finite),
        ),
    }
}

/// Accessible summary for a categorical bar chart:
/// `"<kind>, N bars, min <lo>, max <hi>, categories: a, b, c, +K more"`.
fn describe_bars(kind: &str, bars: &[(String, f32)]) -> String {
    let values: Vec<f32> = bars.iter().map(|(_, v)| *v).collect();
    match finite_min_max(&values) {
        None => format!("{kind}, no data"),
        Some((lo, hi)) => {
            let cats: Vec<String> = bars.iter().map(|(l, _)| l.clone()).collect();
            format!(
                "{kind}, {}, min {}, max {}, categories: {}",
                count_noun(bars.len(), "bar"),
                fmt_num(lo),
                fmt_num(hi),
                bounded_labels(&cats, 4),
            )
        }
    }
}

/// Accessible summary for a multi-series line chart:
/// `"<kind>, K series, N points each, min <lo>, max <hi>"`. When series have
/// different lengths the point count becomes a `"<lo>–<hi> points each"` range.
fn describe_multi(kind: &str, series: &[(String, Vec<f32>)]) -> String {
    let all: Vec<f32> = series.iter().flat_map(|(_, v)| v.iter().copied()).collect();
    let Some((lo, hi)) = finite_min_max(&all) else {
        return format!("{kind}, no data");
    };
    let counts: Vec<usize> = series
        .iter()
        .map(|(_, v)| v.iter().filter(|x| x.is_finite()).count())
        .collect();
    let (cmin, cmax) = counts
        .iter()
        .copied()
        .fold((usize::MAX, 0usize), |(lo, hi), c| (lo.min(c), hi.max(c)));
    let points = if cmin == cmax {
        format!("{} each", count_noun(cmax, "point"))
    } else {
        format!("{cmin}\u{2013}{cmax} points each")
    };
    // "series" is invariant in the plural, so it is not run through count_noun.
    format!(
        "{kind}, {} series, {points}, min {}, max {}",
        series.len(),
        fmt_num(lo),
        fmt_num(hi),
    )
}

/// Accessible summary for a scatter chart:
/// `"scatter chart, N points, x <lo>–<hi>, y <lo>–<hi>"`.
fn describe_scatter(points: &[(f32, f32)]) -> String {
    let xs: Vec<f32> = points.iter().map(|(x, _)| *x).collect();
    let ys: Vec<f32> = points.iter().map(|(_, y)| *y).collect();
    match (finite_min_max(&xs), finite_min_max(&ys)) {
        (Some((xlo, xhi)), Some((ylo, yhi))) => format!(
            "scatter chart, {}, x {}\u{2013}{}, y {}\u{2013}{}",
            count_noun(points.len(), "point"),
            fmt_num(xlo),
            fmt_num(xhi),
            fmt_num(ylo),
            fmt_num(yhi),
        ),
        _ => "scatter chart, no data".to_string(),
    }
}

/// Accessible summary for a pie / donut chart, enumerating the leading slices
/// with their share: `"<kind>, N slices: A 50%, B 30%, +K more"`.
fn describe_pie(kind: &str, segments: &[(String, f32)]) -> String {
    let total: f32 = segments.iter().map(|(_, v)| *v).sum();
    if segments.is_empty() || total <= f32::EPSILON {
        return format!("{kind}, no data");
    }
    const SHOWN: usize = 4;
    let mut parts: Vec<String> = segments
        .iter()
        .take(SHOWN)
        .map(|(label, v)| format!("{label} {}%", fmt_num((v / total * 100.0).round())))
        .collect();
    if segments.len() > SHOWN {
        parts.push(format!("+{} more", segments.len() - SHOWN));
    }
    format!(
        "{kind}, {}: {}",
        count_noun(segments.len(), "slice"),
        parts.join(", "),
    )
}

/// Accessible summary for a stacked / grouped bar grid:
/// `"<kind>, K series × N groups, <peak_label> <peak>"`.
fn describe_grid(
    kind: &str,
    n_series: usize,
    n_groups: usize,
    peak_label: &str,
    peak: f32,
) -> String {
    if n_series == 0 || n_groups == 0 {
        return format!("{kind}, no data");
    }
    format!(
        "{kind}, {n_series} series \u{00d7} {}, {peak_label} {}",
        count_noun(n_groups, "group"),
        fmt_num(peak),
    )
}

/// Append opt-in axis-title context to a chart's accessible description, so a
/// titled chart announces what its axes mean.
fn append_axis_titles(base: String, x: Option<&str>, y: Option<&str>) -> String {
    match (x, y) {
        (Some(x), Some(y)) => format!("{base}, x-axis: {x}, y-axis: {y}"),
        (Some(x), None) => format!("{base}, x-axis: {x}"),
        (None, Some(y)) => format!("{base}, y-axis: {y}"),
        (None, None) => base,
    }
}

// ── Path construction helpers ─────────────────────────────────────────────────

/// A filled pie slice in a (1, 1) viewbox: centre (0.5, 0.5), outer radius
/// `r`, sweeping from `start` by `sweep` radians (positive = clockwise on
/// screen, y-down).
fn pie_slice(start: f64, sweep: f64, r: f64) -> BezPath {
    let (cx, cy) = (0.5, 0.5);
    let mut bez = BezPath::new();
    if sweep.abs() < 1e-10 || r <= 0.0 {
        return bez;
    }
    bez.move_to((cx, cy));
    bez.line_to((cx + r * start.cos(), cy + r * start.sin()));
    Arc::new((cx, cy), (r, r), start, sweep, 0.0)
        .to_cubic_beziers(0.001, |p1, p2, p3| bez.curve_to(p1, p2, p3));
    bez.close_path();
    bez
}

/// A filled donut slice in a (1, 1) viewbox: outer radius `r_outer`, inner
/// hole radius `r_inner`, sweeping from `start` by `sweep` radians.
fn donut_slice(start: f64, sweep: f64, r_outer: f64, r_inner: f64) -> BezPath {
    let (cx, cy) = (0.5, 0.5);
    let mut bez = BezPath::new();
    if sweep.abs() < 1e-10 || r_outer <= 0.0 || r_inner >= r_outer {
        return bez;
    }
    // Outer arc start-point
    bez.move_to((cx + r_outer * start.cos(), cy + r_outer * start.sin()));
    // Outer arc forward
    Arc::new((cx, cy), (r_outer, r_outer), start, sweep, 0.0)
        .to_cubic_beziers(0.001, |p1, p2, p3| bez.curve_to(p1, p2, p3));
    // Line to inner arc end-point
    let end = start + sweep;
    bez.line_to((cx + r_inner * end.cos(), cy + r_inner * end.sin()));
    // Inner arc reversed
    Arc::new((cx, cy), (r_inner, r_inner), end, -sweep, 0.0)
        .to_cubic_beziers(0.001, |p1, p2, p3| bez.curve_to(p1, p2, p3));
    bez.close_path();
    bez
}

/// A filled area path for a normalized series in a `(n-1) × 1` viewbox:
/// polygon from (0, 1) along the data points, then returning to (n-1, 1).
fn area_fill_path(points: &[f32]) -> BezPath {
    let mut bez = BezPath::new();
    let n = points.len();
    if n < 2 {
        return bez;
    }
    bez.move_to((0.0, 1.0));
    for (i, &v) in points.iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
        bez.line_to((i as f64, f64::from(1.0 - v)));
    }
    #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
    bez.line_to(((n - 1) as f64, 1.0));
    bez.close_path();
    bez
}

// ── Axis layout ───────────────────────────────────────────────────────────────

/// Left clearance reserved for y-axis tick labels.
const AX_LEFT: f32 = 42.0;
/// Bottom clearance reserved for x-axis tick labels.
const AX_BOTTOM: f32 = 22.0;
/// Top clearance — breathing room above the highest gridline.
const AX_TOP: f32 = 6.0;
/// Right clearance.
const AX_RIGHT: f32 = 6.0;
/// Extra clearance reserved for an opt-in axis title (added to the left margin
/// for a y-title, to the bottom for an x-title). Zero when no title is set, so
/// untitled charts lay out — and render — byte-identically to before.
const AX_TITLE_GAP: f32 = 20.0;

/// Pre-computed axis geometry and tick values for one chart.
struct AxisLayout {
    /// X offset (left edge of plot area within the chart).
    plot_x: f32,
    /// Y offset (top edge of plot area within the chart).
    plot_y: f32,
    /// Pixel width of the plot area.
    plot_w: f32,
    /// Pixel height of the plot area.
    plot_h: f32,
    /// Computed nice tick values.
    ticks: Vec<f32>,
    /// Step between adjacent ticks (for label formatting).
    step: f32,
    /// Minimum value represented by the axis (first tick).
    axis_lo: f32,
    /// Span of the axis (last tick minus first tick).
    axis_range: f32,
}

impl AxisLayout {
    /// Build from chart outer dimensions and the data value range.
    fn from_data(w: f32, h: f32, lo: f32, hi: f32, target_ticks: usize) -> Self {
        Self::from_data_inset(w, h, lo, hi, target_ticks, 0.0, 0.0)
    }

    /// Build with extra `inset_left` / `inset_bottom` clearance reserved for
    /// opt-in axis titles. With both insets `0.0` this is identical to
    /// [`AxisLayout::from_data`], so untitled charts are byte-identical.
    fn from_data_inset(
        w: f32,
        h: f32,
        lo: f32,
        hi: f32,
        target_ticks: usize,
        inset_left: f32,
        inset_bottom: f32,
    ) -> Self {
        let lo = if lo.is_finite() { lo } else { 0.0 };
        let hi = if hi.is_finite() { hi } else { 1.0 };
        let (lo, hi) = if (hi - lo).abs() < f32::EPSILON {
            (lo - 1.0, lo + 1.0)
        } else if lo > hi {
            (hi, lo)
        } else {
            (lo, hi)
        };

        let ticks = nice_ticks(lo, hi, target_ticks);
        let step = if ticks.len() >= 2 {
            (ticks[1] - ticks[0]).abs().max(f32::EPSILON)
        } else {
            1.0
        };
        let axis_lo = ticks.first().copied().unwrap_or(lo);
        let axis_hi = ticks.last().copied().unwrap_or(hi);
        let axis_range = (axis_hi - axis_lo).max(f32::EPSILON);

        Self {
            plot_x: AX_LEFT + inset_left,
            plot_y: AX_TOP,
            plot_w: (w - AX_LEFT - inset_left - AX_RIGHT).max(1.0),
            plot_h: (h - AX_TOP - AX_BOTTOM - inset_bottom).max(1.0),
            ticks,
            step,
            axis_lo,
            axis_range,
        }
    }

    /// Pixel y-position (from chart top) for a data value.
    fn y_of(&self, val: f32) -> f32 {
        let frac = ((val - self.axis_lo) / self.axis_range).clamp(0.0, 1.0);
        self.plot_y + self.plot_h * (1.0 - frac)
    }

    /// Pixel x-position for a data value within [x_lo, x_lo + x_range].
    fn x_of(&self, val: f32, x_lo: f32, x_range: f32) -> f32 {
        let frac = ((val - x_lo) / x_range.max(f32::EPSILON)).clamp(0.0, 1.0);
        self.plot_x + self.plot_w * frac
    }
}

// ── Shared axis-drawing helpers ───────────────────────────────────────────────

/// Append horizontal gridlines, y-axis line, and tick labels to `chart`
/// (a stack element). Returns the augmented chart.
fn with_y_axis<Msg>(mut chart: Element<Msg>, ax: &AxisLayout) -> Element<Msg> {
    // Vertical axis line on the left edge of the plot area.
    chart = chart.child(
        div()
            .absolute()
            .top(ax.plot_y)
            .left(ax.plot_x - 1.0)
            .w(1.0)
            .h(ax.plot_h + 1.0)
            .themed(|t: &Theme, s| s.bg(t.border)),
    );

    for &tick in &ax.ticks {
        let y = ax.y_of(tick);
        let label = fmt_tick(tick, ax.step);

        // Horizontal gridline across the full plot width.
        chart = chart.child(
            div()
                .absolute()
                .top(y)
                .left(ax.plot_x)
                .w(ax.plot_w)
                .h(1.0)
                .themed(|t: &Theme, s| s.bg(t.border_subtle)),
        );

        // Tick label to the left of the axis, right-aligned.
        let label_top = (y - 8.0).max(0.0);
        chart = chart.child(
            text(label)
                .size(TextSize::Xs)
                .tabular()
                .absolute()
                .top(label_top)
                .left(0.0)
                .w(ax.plot_x - 4.0)
                .text_align(TextAlign::End)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        );
    }
    chart
}

/// Append x-axis baseline and categorical labels below the plot area.
fn with_x_labels<Msg>(mut chart: Element<Msg>, labels: &[String], ax: &AxisLayout) -> Element<Msg> {
    // X-axis baseline.
    chart = chart.child(
        div()
            .absolute()
            .top(ax.plot_y + ax.plot_h)
            .left(ax.plot_x - 1.0)
            .w(ax.plot_w + 1.0)
            .h(1.0)
            .themed(|t: &Theme, s| s.bg(t.border)),
    );

    let n = labels.len();
    if n == 0 {
        return chart;
    }

    #[expect(clippy::cast_precision_loss, reason = "label counts are small")]
    let slot = ax.plot_w / n as f32;
    for (i, label) in labels.iter().enumerate() {
        #[expect(clippy::cast_precision_loss, reason = "label counts are small")]
        let cx = ax.plot_x + (i as f32 + 0.5) * slot;
        let label = label.clone();
        chart = chart.child(
            text(label)
                .size(TextSize::Xs)
                .absolute()
                .top(ax.plot_y + ax.plot_h + 4.0)
                .left((cx - 20.0).max(0.0))
                .w(40.0)
                .text_align(TextAlign::Center)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        );
    }
    chart
}

/// Append x-axis tick labels (numeric) for a scatter / area chart.
fn with_x_ticks<Msg>(
    mut chart: Element<Msg>,
    ax: &AxisLayout,
    x_ticks: &[(f32, f32)],
) -> Element<Msg> {
    // X-axis baseline.
    chart = chart.child(
        div()
            .absolute()
            .top(ax.plot_y + ax.plot_h)
            .left(ax.plot_x - 1.0)
            .w(ax.plot_w + 1.0)
            .h(1.0)
            .themed(|t: &Theme, s| s.bg(t.border)),
    );

    for &(val, x_px) in x_ticks {
        let step = if x_ticks.len() >= 2 {
            (x_ticks[1].0 - x_ticks[0].0).abs()
        } else {
            1.0
        };
        let label = fmt_tick(val, step);
        chart = chart.child(
            text(label)
                .size(TextSize::Xs)
                .tabular()
                .absolute()
                .top(ax.plot_y + ax.plot_h + 4.0)
                .left((x_px - 20.0).max(0.0))
                .w(40.0)
                .text_align(TextAlign::Center)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        );
    }
    chart
}

/// Left inset to reserve for a y-axis title (zero when none is set).
fn left_inset(y_title: Option<&str>) -> f32 {
    if y_title.is_some() { AX_TITLE_GAP } else { 0.0 }
}

/// Bottom inset to reserve for an x-axis title (zero when none is set).
fn bottom_inset(x_title: Option<&str>) -> f32 {
    if x_title.is_some() { AX_TITLE_GAP } else { 0.0 }
}

/// Append opt-in axis titles to a chart. The x-title sits centered below the
/// tick labels; the y-title is rotated a quarter turn along the left edge,
/// reading bottom-to-top. Both use the muted text token and are no-ops when
/// unset (so the room reserved by [`left_inset`]/[`bottom_inset`] is also zero
/// and the render is unchanged). `h` is the plot element's height.
fn with_axis_titles<Msg>(
    mut chart: Element<Msg>,
    ax: &AxisLayout,
    h: f32,
    x_title: Option<&str>,
    y_title: Option<&str>,
) -> Element<Msg> {
    if let Some(title) = x_title {
        chart = chart.child(
            text(title.to_string())
                .size(TextSize::Sm)
                .absolute()
                .top(h - AX_TITLE_GAP + 2.0)
                .left(ax.plot_x)
                .w(ax.plot_w)
                .text_align(TextAlign::Center)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        );
    }
    if let Some(title) = y_title {
        // A horizontal text box as wide as the plot is tall, centered on the
        // left margin, then rotated so it reads bottom-to-top. Rotation is
        // paint-time about the box center, so the pre-rotation overflow to the
        // left is harmless — the clip applies to the rotated pixels.
        let cy = ax.plot_y + ax.plot_h / 2.0;
        let cx = AX_TITLE_GAP / 2.0;
        chart = chart.child(
            text(title.to_string())
                .size(TextSize::Sm)
                .absolute()
                .top(cy - 9.0)
                .left(cx - ax.plot_h / 2.0)
                .w(ax.plot_h)
                .text_align(TextAlign::Center)
                .rotate(-90.0)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        );
    }
    chart
}

/// A horizontal legend row: colored 10×10 swatches + series name text.
fn legend_row<Msg>(labels: &[String]) -> Element<Msg> {
    let mut r = row().gap(12.0).items_center().wrap().px(8.0).pb(4.0);
    for (i, label) in labels.iter().enumerate() {
        let label = label.clone();
        r = r.child(
            row().gap(4.0).items_center().children((
                div()
                    .w(10.0)
                    .h(10.0)
                    .rounded(2.0)
                    .themed(move |t: &Theme, s| s.bg(ChartPalette::series(i, t.mode))),
                text(label)
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            )),
        );
    }
    r
}

/// Standard chart panel background in a stack.
fn chart_bg<Msg>() -> Element<Msg> {
    div()
        .w_full()
        .h_full()
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
}

// ── Simple charts (original API, unchanged) ───────────────────────────────────

/// A tiny inline trend line (defaults 96x24; size with `.w/.h`).
/// Stroked in the theme accent.
pub fn sparkline<Msg>(values: impl IntoIterator<Item = f32>) -> Element<Msg> {
    let values: Vec<f32> = values.into_iter().collect();
    let points = normalized(&values);
    let desc = describe_line("sparkline", &values);
    if points.len() < 2 {
        return div()
            .w(96.0)
            .h(24.0)
            .semantics(Semantics::Image)
            .label(desc);
    }
    #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
    let viewbox = ((points.len() - 1) as f64, 1.0);
    path(polyline(&points), viewbox, Some(0.06))
        .w(96.0)
        .h(24.0)
        .themed(|t: &Theme, s| s.color(t.accent))
        .semantics(Semantics::Image)
        .label(desc)
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
        .label(describe_line("line chart", &values));
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
    let desc = describe_bars("bar chart", &bars);
    row()
        .w(320.0)
        .h(160.0)
        .p(8.0)
        .gap(8.0)
        .items_end()
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
        .semantics(Semantics::Image)
        .label(desc)
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
    let desc = describe_multi("multi-series line chart", &series);
    let mut panel = stack()
        .w(320.0)
        .h(160.0)
        .p(8.0)
        .rounded(6.0)
        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
        .semantics(Semantics::Image)
        .label(desc);
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

// ── LineChartBuilder — line chart with axes, tick labels, gridlines, markers ──

/// A line chart with y-axis ticks, gridlines, tick labels, and optional
/// data-point markers. The accent color drives the series line; axis
/// decorations use border/text-muted theme tokens.
///
/// ```
/// use fenestra_charts::LineChartBuilder;
///
/// let el: fenestra_core::Element<()> = LineChartBuilder::new([2.0, 5.0, 3.0, 8.0])
///     .show_markers()
///     .x_labels(["Mon", "Tue", "Wed", "Thu"])
///     .build();
/// ```
pub struct LineChartBuilder {
    values: Vec<f32>,
    w: f32,
    h: f32,
    show_markers: bool,
    target_ticks: usize,
    x_labels: Option<Vec<String>>,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl LineChartBuilder {
    /// Create a new builder from a series of y-values.
    pub fn new(values: impl IntoIterator<Item = f32>) -> Self {
        Self {
            values: values.into_iter().collect(),
            w: 320.0,
            h: 160.0,
            show_markers: false,
            target_ticks: 5,
            x_labels: None,
            x_title: None,
            y_title: None,
        }
    }

    /// Label the x-axis with a title, centered below the tick labels. Opt-in:
    /// with no title the chart renders byte-identically.
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title, rotated along the left edge. Opt-in:
    /// with no title the chart renders byte-identically.
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Override the chart width in logical pixels.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height in logical pixels.
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Draw a filled dot at each data point.
    pub fn show_markers(mut self) -> Self {
        self.show_markers = true;
        self
    }

    /// Override the target number of y-axis ticks (default 5).
    pub fn target_ticks(mut self, n: usize) -> Self {
        self.target_ticks = n;
        self
    }

    /// Attach category labels to the x-axis.
    pub fn x_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.x_labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    /// Render the chart into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let x_title = self.x_title;
        let y_title = self.y_title;
        let values: Vec<f32> = self.values.into_iter().filter(|v| v.is_finite()).collect();
        let n = values.len();
        let desc = append_axis_titles(
            describe_line("line chart", &values),
            x_title.as_deref(),
            y_title.as_deref(),
        );

        let mut chart = stack()
            .w(self.w)
            .h(self.h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc)
            .child(chart_bg());

        if n < 2 {
            return chart;
        }

        let (lo, hi) = values
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });
        let ax = AxisLayout::from_data_inset(
            self.w,
            self.h,
            lo,
            hi,
            self.target_ticks,
            left_inset(y_title.as_deref()),
            bottom_inset(x_title.as_deref()),
        );

        chart = with_y_axis(chart, &ax);

        let labels = self
            .x_labels
            .unwrap_or_else(|| (0..n).map(|i| i.to_string()).collect());
        chart = with_x_labels(chart, &labels, &ax);

        // Data line
        let points = to_unit_range(&values, ax.axis_lo, ax.axis_lo + ax.axis_range);
        if points.len() >= 2 {
            #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
            let viewbox = ((points.len() - 1) as f64, 1.0);
            chart = chart.child(
                path(polyline(&points), viewbox, Some(0.025))
                    .absolute()
                    .top(ax.plot_y)
                    .left(ax.plot_x)
                    .w(ax.plot_w)
                    .h(ax.plot_h)
                    .themed(|t: &Theme, s| s.color(t.accent)),
            );

            // Markers: 8×8 filled circles at each data point.
            if self.show_markers {
                let pts = points.clone();
                let n_pts = pts.len();
                let plot_x = ax.plot_x;
                let plot_y = ax.plot_y;
                let plot_w = ax.plot_w;
                let plot_h = ax.plot_h;
                for (i, &v) in pts.iter().enumerate() {
                    #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
                    let frac_x = i as f32 / (n_pts - 1).max(1) as f32;
                    let px = plot_x + frac_x * plot_w;
                    let py = plot_y + (1.0 - v) * plot_h;
                    chart = chart.child(
                        div()
                            .w(8.0)
                            .h(8.0)
                            .rounded_full()
                            .absolute()
                            .top(py - 4.0)
                            .left(px - 4.0)
                            .themed(|t: &Theme, s| {
                                s.bg(t.accent).border(2.0, t.elevated_surface(1))
                            }),
                    );
                }
            }
        }

        with_axis_titles(chart, &ax, self.h, x_title.as_deref(), y_title.as_deref())
    }
}

// ── BarChartAxes — bar chart with y-axis, gridlines, optional value labels ────

/// A bar chart with a y-axis (ticks + gridlines), x-axis category labels,
/// and optional per-bar value labels. Uses the theme accent color.
///
/// ```
/// use fenestra_charts::BarChartAxes;
///
/// let el: fenestra_core::Element<()> = BarChartAxes::new([
///     ("Mon", 4.0), ("Tue", 7.0), ("Wed", 3.0),
/// ]).show_values().build();
/// ```
pub struct BarChartAxes {
    bars: Vec<(String, f32)>,
    w: f32,
    h: f32,
    show_values: bool,
    target_ticks: usize,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl BarChartAxes {
    /// Create from `(label, value)` pairs. Negative and non-finite values are
    /// kept for the range calculation but bars with negative values are
    /// rendered anchored to y=0.
    pub fn new(bars: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Self {
        Self {
            bars: bars
                .into_iter()
                .map(|(l, v)| (l.into(), v))
                .filter(|(_, v)| v.is_finite())
                .collect(),
            w: 320.0,
            h: 160.0,
            show_values: false,
            target_ticks: 5,
            x_title: None,
            y_title: None,
        }
    }

    /// Show the numeric value above each bar.
    pub fn show_values(mut self) -> Self {
        self.show_values = true;
        self
    }

    /// Label the x-axis with a title (opt-in; unset renders unchanged).
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title (opt-in; unset renders unchanged).
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Override the chart width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Override the target y-axis tick count.
    pub fn target_ticks(mut self, n: usize) -> Self {
        self.target_ticks = n;
        self
    }

    /// Render the chart into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let x_title = self.x_title;
        let y_title = self.y_title;
        let bars = self.bars;
        let n = bars.len();
        let desc = append_axis_titles(
            describe_bars("bar chart", &bars),
            x_title.as_deref(),
            y_title.as_deref(),
        );

        let mut chart = stack()
            .w(self.w)
            .h(self.h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc)
            .child(chart_bg());

        if n == 0 {
            return chart;
        }

        let data_max = bars.iter().map(|(_, v)| *v).fold(0.0_f32, f32::max);
        let data_lo = 0.0_f32.min(bars.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min));
        let ax = AxisLayout::from_data_inset(
            self.w,
            self.h,
            data_lo,
            data_max,
            self.target_ticks,
            left_inset(y_title.as_deref()),
            bottom_inset(x_title.as_deref()),
        );

        chart = with_y_axis(chart, &ax);
        let x_labels: Vec<String> = bars.iter().map(|(l, _)| l.clone()).collect();
        chart = with_x_labels(chart, &x_labels, &ax);

        // Draw bars using absolute positioning.
        #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
        let slot = ax.plot_w / n as f32;
        let gap = (slot * 0.2).max(2.0);
        let bar_w = (slot - gap).max(1.0);
        let baseline_y = ax.y_of(0.0);

        for (i, (_, val)) in bars.iter().enumerate() {
            #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
            let slot_left = ax.plot_x + i as f32 * slot;
            let bar_left = slot_left + gap * 0.5;
            let bar_top = ax.y_of(*val);
            let bar_h = (baseline_y - bar_top).abs().max(1.0);
            let actual_top = bar_top.min(baseline_y);
            let val_copy = *val;

            chart = chart.child(
                div()
                    .absolute()
                    .top(actual_top)
                    .left(bar_left)
                    .w(bar_w)
                    .h(bar_h)
                    .rounded(2.0)
                    .themed(|t: &Theme, s| s.bg(t.accent)),
            );

            if self.show_values {
                let label = fmt_tick(val_copy, ax.step);
                let label_top = (actual_top - 14.0).max(0.0);
                chart = chart.child(
                    text(label)
                        .size(TextSize::Xs)
                        .tabular()
                        .absolute()
                        .top(label_top)
                        .left(bar_left)
                        .w(bar_w)
                        .text_align(TextAlign::Center)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                );
            }
        }

        with_axis_titles(chart, &ax, self.h, x_title.as_deref(), y_title.as_deref())
    }
}

// ── MultiSeriesChart — multi-line with axes and legend ────────────────────────

/// A multi-series line chart with y-axis ticks, gridlines, and a legend row
/// showing series names and their palette swatches.
///
/// ```
/// use fenestra_charts::MultiSeriesChart;
///
/// let el: fenestra_core::Element<()> = MultiSeriesChart::new([
///     ("cpu", vec![12.0, 18.0, 9.0, 26.0]),
///     ("mem", vec![20.0, 16.0, 22.0, 19.0]),
/// ]).build();
/// ```
pub struct MultiSeriesChart {
    series: Vec<(String, Vec<f32>)>,
    w: f32,
    h: f32,
    show_markers: bool,
    target_ticks: usize,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl MultiSeriesChart {
    /// Create from `(name, values)` pairs.
    pub fn new(
        series: impl IntoIterator<Item = (impl Into<String>, impl IntoIterator<Item = f32>)>,
    ) -> Self {
        Self {
            series: series
                .into_iter()
                .map(|(l, v)| (l.into(), v.into_iter().collect()))
                .collect(),
            w: 320.0,
            h: 192.0, // extra height for legend
            show_markers: false,
            target_ticks: 5,
            x_title: None,
            y_title: None,
        }
    }

    /// Draw a filled dot at each data point on every series.
    pub fn show_markers(mut self) -> Self {
        self.show_markers = true;
        self
    }

    /// Label the x-axis with a title (opt-in; unset renders unchanged).
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title (opt-in; unset renders unchanged).
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Override the chart width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height (legend is additional).
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Override the target y-axis tick count.
    pub fn target_ticks(mut self, n: usize) -> Self {
        self.target_ticks = n;
        self
    }

    /// Render the chart (plot area + legend below) into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let x_title = self.x_title;
        let y_title = self.y_title;
        let series = self.series;
        let labels: Vec<String> = series.iter().map(|(l, _)| l.clone()).collect();
        let desc = append_axis_titles(
            describe_multi("multi-series line chart", &series),
            x_title.as_deref(),
            y_title.as_deref(),
        );

        // Legend height reserve
        let legend_h = 24.0;
        let plot_h = (self.h - legend_h).max(60.0);

        // Compute shared data range across all series.
        let (global_lo, global_hi) = series
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v), hi.max(v))
            });

        let mut plot = stack()
            .w(self.w)
            .h(plot_h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc.clone())
            .child(chart_bg());

        if global_lo.is_finite() && global_hi.is_finite() {
            let ax = AxisLayout::from_data_inset(
                self.w,
                plot_h,
                global_lo,
                global_hi,
                self.target_ticks,
                left_inset(y_title.as_deref()),
                bottom_inset(x_title.as_deref()),
            );
            plot = with_y_axis(plot, &ax);

            // X baseline
            plot = plot.child(
                div()
                    .absolute()
                    .top(ax.plot_y + ax.plot_h)
                    .left(ax.plot_x - 1.0)
                    .w(ax.plot_w + 1.0)
                    .h(1.0)
                    .themed(|t: &Theme, s| s.bg(t.border)),
            );

            for (si, (_, values)) in series.iter().enumerate() {
                let points = to_unit_range(values, ax.axis_lo, ax.axis_lo + ax.axis_range);
                if points.len() < 2 {
                    continue;
                }
                #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
                let viewbox = ((points.len() - 1) as f64, 1.0);
                plot = plot.child(
                    path(polyline(&points), viewbox, Some(0.025))
                        .absolute()
                        .top(ax.plot_y)
                        .left(ax.plot_x)
                        .w(ax.plot_w)
                        .h(ax.plot_h)
                        .themed(move |t: &Theme, s| s.color(ChartPalette::series(si, t.mode))),
                );

                if self.show_markers {
                    let n_pts = points.len();
                    let plot_x = ax.plot_x;
                    let plot_y = ax.plot_y;
                    let plot_w_m = ax.plot_w;
                    let plot_h_m = ax.plot_h;
                    for (i, &v) in points.iter().enumerate() {
                        #[expect(
                            clippy::cast_precision_loss,
                            reason = "chart point counts are small"
                        )]
                        let frac_x = i as f32 / (n_pts - 1).max(1) as f32;
                        let px = plot_x + frac_x * plot_w_m;
                        let py = plot_y + (1.0 - v) * plot_h_m;
                        plot = plot.child(
                            div()
                                .w(6.0)
                                .h(6.0)
                                .rounded_full()
                                .absolute()
                                .top(py - 3.0)
                                .left(px - 3.0)
                                .themed(move |t: &Theme, s| {
                                    s.bg(ChartPalette::series(si, t.mode))
                                        .border(2.0, t.elevated_surface(1))
                                }),
                        );
                    }
                }
            }

            plot = with_axis_titles(plot, &ax, plot_h, x_title.as_deref(), y_title.as_deref());
        }

        col()
            .gap(0.0)
            .semantics(Semantics::Image)
            .label(desc)
            .children((plot, legend_row::<Msg>(&labels)))
    }
}

// ── area_chart ────────────────────────────────────────────────────────────────

/// An area chart: a filled region under the series line, stroked on top.
/// Uses the theme accent at full opacity for the stroke and 20% opacity for
/// the fill. Defaults 320×160; axes + ticks shown. Size with `.w/.h` after
/// calling this is not supported — use [`LineChartBuilder`] for that.
pub fn area_chart<Msg>(values: impl IntoIterator<Item = f32>) -> Element<Msg> {
    let values: Vec<f32> = values.into_iter().filter(|v| v.is_finite()).collect();
    let n = values.len();

    let w = 320.0_f32;
    let h = 160.0_f32;

    let mut chart = stack()
        .w(w)
        .h(h)
        .rounded(6.0)
        .overflow_hidden()
        .semantics(Semantics::Image)
        .label(describe_line("area chart", &values))
        .child(chart_bg());

    if n < 2 {
        return chart;
    }

    let (lo, hi) = values
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
    let ax = AxisLayout::from_data(w, h, lo, hi, 5);
    chart = with_y_axis(chart, &ax);
    chart = with_x_labels(chart, &[], &ax);

    let points = to_unit_range(&values, ax.axis_lo, ax.axis_lo + ax.axis_range);
    if points.len() >= 2 {
        #[expect(clippy::cast_precision_loss, reason = "chart point counts are small")]
        let viewbox = ((points.len() - 1) as f64, 1.0);

        // Filled area
        chart = chart.child(
            path(area_fill_path(&points), viewbox, None)
                .absolute()
                .top(ax.plot_y)
                .left(ax.plot_x)
                .w(ax.plot_w)
                .h(ax.plot_h)
                .themed(|t: &Theme, s| s.color(t.accent.with_alpha(0.20))),
        );

        // Stroke on top
        chart = chart.child(
            path(polyline(&points), viewbox, Some(0.025))
                .absolute()
                .top(ax.plot_y)
                .left(ax.plot_x)
                .w(ax.plot_w)
                .h(ax.plot_h)
                .themed(|t: &Theme, s| s.color(t.accent)),
        );
    }

    chart
}

// ── ScatterChart ──────────────────────────────────────────────────────────────

/// A scatter chart with x and y axes, tick labels on both axes, and a dot at
/// each `(x, y)` data point.
///
/// ```
/// use fenestra_charts::ScatterChart;
///
/// let el: fenestra_core::Element<()> = ScatterChart::new([
///     (1.0_f32, 2.0_f32), (3.0, 1.5), (2.0, 4.0), (4.0, 3.0),
/// ]).build();
/// ```
pub struct ScatterChart {
    points: Vec<(f32, f32)>,
    w: f32,
    h: f32,
    target_ticks: usize,
    dot_size: f32,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl ScatterChart {
    /// Create from `(x, y)` pairs. Non-finite pairs are skipped.
    pub fn new(points: impl IntoIterator<Item = (f32, f32)>) -> Self {
        Self {
            points: points
                .into_iter()
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .collect(),
            w: 320.0,
            h: 240.0,
            target_ticks: 5,
            dot_size: 6.0,
            x_title: None,
            y_title: None,
        }
    }

    /// Label the x-axis with a title (opt-in; unset renders unchanged).
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title (opt-in; unset renders unchanged).
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Override the chart width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Override the dot diameter in logical pixels (default 6).
    pub fn dot_size(mut self, px: f32) -> Self {
        self.dot_size = px.max(2.0);
        self
    }

    /// Render the chart into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let x_title = self.x_title;
        let y_title = self.y_title;
        let desc = append_axis_titles(
            describe_scatter(&self.points),
            x_title.as_deref(),
            y_title.as_deref(),
        );
        let mut chart = stack()
            .w(self.w)
            .h(self.h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc)
            .child(chart_bg());

        if self.points.is_empty() {
            return chart;
        }

        let (x_lo, x_hi) = self
            .points
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &(x, _)| {
                (lo.min(x), hi.max(x))
            });
        let (y_lo, y_hi) = self
            .points
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &(_, y)| {
                (lo.min(y), hi.max(y))
            });

        let ax = AxisLayout::from_data_inset(
            self.w,
            self.h,
            y_lo,
            y_hi,
            self.target_ticks,
            left_inset(y_title.as_deref()),
            bottom_inset(x_title.as_deref()),
        );
        chart = with_y_axis(chart, &ax);

        // X axis ticks
        let x_ticks = nice_ticks(x_lo, x_hi, self.target_ticks);
        let x_tick_lo = x_ticks.first().copied().unwrap_or(x_lo);
        let x_tick_hi = x_ticks.last().copied().unwrap_or(x_hi);
        let x_range = (x_tick_hi - x_tick_lo).max(f32::EPSILON);
        let x_tick_step = if x_ticks.len() >= 2 {
            (x_ticks[1] - x_ticks[0]).abs()
        } else {
            1.0
        };
        let x_tick_pairs: Vec<(f32, f32)> = x_ticks
            .iter()
            .map(|&v| (v, ax.x_of(v, x_tick_lo, x_range)))
            .collect();
        chart = with_x_ticks(chart, &ax, &x_tick_pairs);

        // Vertical gridlines at x ticks
        for &(_, x_px) in &x_tick_pairs {
            chart = chart.child(
                div()
                    .absolute()
                    .top(ax.plot_y)
                    .left(x_px)
                    .w(1.0)
                    .h(ax.plot_h)
                    .themed(|t: &Theme, s| s.bg(t.border_subtle)),
            );
        }

        // X-axis tick value labels
        let _ = x_tick_step; // used above indirectly via x_tick_pairs

        // Dots
        let dot_r = self.dot_size * 0.5;
        for &(x, y) in &self.points {
            let px = ax.x_of(x, x_tick_lo, x_range);
            let py = ax.y_of(y);
            chart = chart.child(
                div()
                    .w(self.dot_size)
                    .h(self.dot_size)
                    .rounded_full()
                    .absolute()
                    .top(py - dot_r)
                    .left(px - dot_r)
                    .themed(|t: &Theme, s| s.bg(t.accent)),
            );
        }

        with_axis_titles(chart, &ax, self.h, x_title.as_deref(), y_title.as_deref())
    }
}

// ── PieChart — pie and donut charts with legend ───────────────────────────────

/// A pie or donut chart with a legend. Segments are drawn in categorical
/// palette colors. Set a hole radius (0–1) with `.donut(fraction)` to
/// convert to a donut.
///
/// ```
/// use fenestra_charts::PieChart;
///
/// let pie: fenestra_core::Element<()> = PieChart::new([
///     ("Alpha", 40.0_f32), ("Beta", 30.0), ("Gamma", 30.0),
/// ]).build();
///
/// let donut: fenestra_core::Element<()> = PieChart::new([
///     ("Alpha", 40.0_f32), ("Beta", 30.0), ("Gamma", 30.0),
/// ]).donut(0.55).build();
/// ```
pub struct PieChart {
    segments: Vec<(String, f32)>,
    size: f32,
    hole_frac: f64,
}

impl PieChart {
    /// Create from `(label, value)` pairs. Negative and non-finite values are
    /// skipped.
    pub fn new(segments: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Self {
        Self {
            segments: segments
                .into_iter()
                .map(|(l, v)| (l.into(), v))
                .filter(|(_, v)| v.is_finite() && *v > 0.0)
                .collect(),
            size: 200.0,
            hole_frac: 0.0,
        }
    }

    /// Convert to a donut chart by specifying the inner hole as a fraction of
    /// the outer radius (0.0 = solid pie, 0.55 = typical donut).
    pub fn donut(mut self, hole_frac: f64) -> Self {
        self.hole_frac = hole_frac.clamp(0.0, 0.9);
        self
    }

    /// Override the pie diameter in logical pixels (default 200).
    pub fn size(mut self, px: f32) -> Self {
        self.size = px.max(40.0);
        self
    }

    /// Render the chart (pie + legend below) into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let segs = self.segments;
        let labels: Vec<String> = segs.iter().map(|(l, _)| l.clone()).collect();
        let total: f32 = segs.iter().map(|(_, v)| *v).sum();
        let is_donut = self.hole_frac > 0.01;

        let kind_label = if is_donut { "donut chart" } else { "pie chart" };
        let desc = describe_pie(kind_label, &segs);

        let mut pie = stack()
            .w(self.size)
            .h(self.size)
            .semantics(Semantics::Image)
            .label(desc.clone());

        if total > f32::EPSILON && !segs.is_empty() {
            let r_outer = 0.47;
            let r_inner = if is_donut {
                r_outer * self.hole_frac
            } else {
                0.0
            };

            let mut angle = -FRAC_PI_2; // Start at 12 o'clock
            for (i, (_, val)) in segs.iter().enumerate() {
                let sweep = f64::from(val / total) * TAU;
                let slice = if is_donut {
                    donut_slice(angle, sweep, r_outer, r_inner)
                } else {
                    pie_slice(angle, sweep, r_outer)
                };
                angle += sweep;

                pie = pie.child(
                    path(slice, (1.0, 1.0), None)
                        .w_full()
                        .h_full()
                        .themed(move |t: &Theme, s| s.color(ChartPalette::series(i, t.mode))),
                );
            }

            // Donut centre decoration: subtle surface fill
            if is_donut {
                let hole_px = self.size * r_inner as f32;
                let offset = (self.size - hole_px) * 0.5;
                pie = pie.child(
                    div()
                        .w(hole_px)
                        .h(hole_px)
                        .rounded_full()
                        .absolute()
                        .top(offset)
                        .left(offset)
                        .themed(|t: &Theme, s| s.bg(t.elevated_surface(1))),
                );
            }
        }

        col()
            .gap(0.0)
            .items_center()
            .semantics(Semantics::Image)
            .label(desc)
            .children((pie, legend_row::<Msg>(&labels)))
    }
}

// ── StackedBarChart ───────────────────────────────────────────────────────────

/// A stacked bar chart: multiple series stacked in each category bar, each
/// series in its categorical palette color.
///
/// ```
/// use fenestra_charts::StackedBarChart;
///
/// let el: fenestra_core::Element<()> = StackedBarChart::new(
///     ["Mon", "Tue", "Wed"],
///     [
///         ("web",  vec![3.0_f32, 5.0, 4.0]),
///         ("api",  vec![2.0_f32, 3.0, 6.0]),
///         ("db",   vec![1.0_f32, 2.0, 1.5]),
///     ],
/// ).build();
/// ```
pub struct StackedBarChart {
    categories: Vec<String>,
    series: Vec<(String, Vec<f32>)>,
    w: f32,
    h: f32,
    target_ticks: usize,
    show_values: bool,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl StackedBarChart {
    /// Create from category labels and `(series_name, per-category values)`
    /// pairs. Non-finite values are treated as zero.
    pub fn new(
        categories: impl IntoIterator<Item = impl Into<String>>,
        series: impl IntoIterator<Item = (impl Into<String>, impl IntoIterator<Item = f32>)>,
    ) -> Self {
        Self {
            categories: categories.into_iter().map(Into::into).collect(),
            series: series
                .into_iter()
                .map(|(l, v)| (l.into(), v.into_iter().collect()))
                .collect(),
            w: 320.0,
            h: 200.0,
            target_ticks: 5,
            show_values: false,
            x_title: None,
            y_title: None,
        }
    }

    /// Override the chart width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Show a total value label above each bar.
    pub fn show_values(mut self) -> Self {
        self.show_values = true;
        self
    }

    /// Label the x-axis with a title (opt-in; unset renders unchanged).
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title (opt-in; unset renders unchanged).
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Render the chart into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let x_title = self.x_title;
        let y_title = self.y_title;
        let n_cats = self.categories.len();
        let n_series = self.series.len();
        let legend_h = 28.0;
        let plot_total_h = (self.h - legend_h).max(60.0);

        let series_names: Vec<String> = self.series.iter().map(|(l, _)| l.clone()).collect();

        // Compute per-category totals and global max
        let totals: Vec<f32> = (0..n_cats)
            .map(|ci| {
                self.series
                    .iter()
                    .map(|(_, vals)| vals.get(ci).copied().unwrap_or(0.0).max(0.0))
                    .filter(|v| v.is_finite())
                    .sum::<f32>()
            })
            .collect();
        let max_total = totals.iter().copied().fold(0.0_f32, f32::max);

        let desc = append_axis_titles(
            describe_grid(
                "stacked bar chart",
                n_series,
                n_cats,
                "max total",
                max_total,
            ),
            x_title.as_deref(),
            y_title.as_deref(),
        );

        let ax = AxisLayout::from_data_inset(
            self.w,
            plot_total_h,
            0.0,
            max_total,
            self.target_ticks,
            left_inset(y_title.as_deref()),
            bottom_inset(x_title.as_deref()),
        );

        let mut plot = stack()
            .w(self.w)
            .h(plot_total_h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc.clone())
            .child(chart_bg());

        plot = with_y_axis(plot, &ax);
        plot = with_x_labels(plot, &self.categories, &ax);

        #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
        let slot = ax.plot_w / n_cats.max(1) as f32;
        let gap = (slot * 0.2).max(2.0);
        let bar_w = (slot - gap).max(1.0);

        for ci in 0..n_cats {
            #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
            let bar_left = ax.plot_x + ci as f32 * slot + gap * 0.5;
            let mut stack_y = ax.plot_y + ax.plot_h; // start at baseline

            for (si, (_, vals)) in self.series.iter().enumerate() {
                let val = vals.get(ci).copied().unwrap_or(0.0).max(0.0);
                if !val.is_finite() || val <= 0.0 {
                    continue;
                }
                let seg_h = (val / ax.axis_range) * ax.plot_h;
                let seg_top = (stack_y - seg_h).max(ax.plot_y);
                let actual_h = (stack_y - seg_top).max(1.0);
                stack_y = seg_top;
                plot = plot.child(
                    div()
                        .absolute()
                        .top(seg_top)
                        .left(bar_left)
                        .w(bar_w)
                        .h(actual_h)
                        .themed(move |t: &Theme, s| s.bg(ChartPalette::series(si, t.mode))),
                );
            }

            if self.show_values && max_total > f32::EPSILON {
                let total = totals.get(ci).copied().unwrap_or(0.0);
                let label = fmt_tick(total, ax.step);
                let label_top = (stack_y - 14.0).max(0.0);
                plot = plot.child(
                    text(label)
                        .size(TextSize::Xs)
                        .tabular()
                        .absolute()
                        .top(label_top)
                        .left(bar_left)
                        .w(bar_w)
                        .text_align(TextAlign::Center)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                );
            }
        }

        plot = with_axis_titles(
            plot,
            &ax,
            plot_total_h,
            x_title.as_deref(),
            y_title.as_deref(),
        );

        col()
            .gap(0.0)
            .semantics(Semantics::Image)
            .label(desc)
            .children((plot, legend_row::<Msg>(&series_names)))
    }
}

// ── GroupedBarChart ───────────────────────────────────────────────────────────

/// A grouped (clustered) bar chart: each category contains one sub-bar per
/// series, all in their categorical palette color.
///
/// ```
/// use fenestra_charts::GroupedBarChart;
///
/// let el: fenestra_core::Element<()> = GroupedBarChart::new(
///     ["Q1", "Q2", "Q3", "Q4"],
///     [
///         ("product_a", vec![3.0_f32, 5.0, 4.0, 6.0]),
///         ("product_b", vec![2.0_f32, 4.0, 3.0, 5.0]),
///     ],
/// ).build();
/// ```
pub struct GroupedBarChart {
    categories: Vec<String>,
    series: Vec<(String, Vec<f32>)>,
    w: f32,
    h: f32,
    target_ticks: usize,
    show_values: bool,
    x_title: Option<String>,
    y_title: Option<String>,
}

impl GroupedBarChart {
    /// Create from category labels and `(series_name, per-category values)`.
    pub fn new(
        categories: impl IntoIterator<Item = impl Into<String>>,
        series: impl IntoIterator<Item = (impl Into<String>, impl IntoIterator<Item = f32>)>,
    ) -> Self {
        Self {
            categories: categories.into_iter().map(Into::into).collect(),
            series: series
                .into_iter()
                .map(|(l, v)| (l.into(), v.into_iter().collect()))
                .collect(),
            w: 320.0,
            h: 200.0,
            target_ticks: 5,
            show_values: false,
            x_title: None,
            y_title: None,
        }
    }

    /// Override the chart width.
    pub fn w(mut self, w: f32) -> Self {
        self.w = w;
        self
    }

    /// Override the chart height.
    pub fn h(mut self, h: f32) -> Self {
        self.h = h;
        self
    }

    /// Show value labels above each sub-bar.
    pub fn show_values(mut self) -> Self {
        self.show_values = true;
        self
    }

    /// Label the x-axis with a title (opt-in; unset renders unchanged).
    pub fn x_title(mut self, title: impl Into<String>) -> Self {
        self.x_title = Some(title.into());
        self
    }

    /// Label the y-axis with a title (opt-in; unset renders unchanged).
    pub fn y_title(mut self, title: impl Into<String>) -> Self {
        self.y_title = Some(title.into());
        self
    }

    /// Render the chart into an [`Element`].
    pub fn build<Msg>(self) -> Element<Msg> {
        let n_cats = self.categories.len();
        let n_series = self.series.len();
        let legend_h = 28.0;
        let plot_total_h = (self.h - legend_h).max(60.0);

        let series_names: Vec<String> = self.series.iter().map(|(l, _)| l.clone()).collect();

        // Global maximum across all series and categories
        let data_max = self
            .series
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .filter(|v| v.is_finite() && *v >= 0.0)
            .fold(0.0_f32, f32::max);

        let x_title = self.x_title;
        let y_title = self.y_title;
        let desc = append_axis_titles(
            describe_grid("grouped bar chart", n_series, n_cats, "max", data_max),
            x_title.as_deref(),
            y_title.as_deref(),
        );

        let ax = AxisLayout::from_data_inset(
            self.w,
            plot_total_h,
            0.0,
            data_max,
            self.target_ticks,
            left_inset(y_title.as_deref()),
            bottom_inset(x_title.as_deref()),
        );

        let mut plot = stack()
            .w(self.w)
            .h(plot_total_h)
            .rounded(6.0)
            .overflow_hidden()
            .semantics(Semantics::Image)
            .label(desc.clone())
            .child(chart_bg());

        plot = with_y_axis(plot, &ax);
        plot = with_x_labels(plot, &self.categories, &ax);

        #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
        let group_slot = ax.plot_w / n_cats.max(1) as f32;
        let group_gap = (group_slot * 0.15).max(2.0);
        let group_w = group_slot - group_gap;
        #[expect(clippy::cast_precision_loss, reason = "series counts are small")]
        let sub_bar_w = if n_series > 0 {
            (group_w / n_series as f32 - 1.0).max(1.0)
        } else {
            group_w
        };

        let baseline_y = ax.y_of(0.0);

        for ci in 0..n_cats {
            #[expect(clippy::cast_precision_loss, reason = "bar counts are small")]
            let group_left = ax.plot_x + ci as f32 * group_slot + group_gap * 0.5;

            for (si, (_, vals)) in self.series.iter().enumerate() {
                #[expect(clippy::cast_precision_loss, reason = "series counts are small")]
                let sub_left = group_left + si as f32 * (sub_bar_w + 1.0);
                let val = vals.get(ci).copied().unwrap_or(0.0);
                if !val.is_finite() {
                    continue;
                }
                let bar_top = ax.y_of(val.max(0.0));
                let bar_h = (baseline_y - bar_top).abs().max(1.0);

                plot = plot.child(
                    div()
                        .absolute()
                        .top(bar_top)
                        .left(sub_left)
                        .w(sub_bar_w)
                        .h(bar_h)
                        .rounded(2.0)
                        .themed(move |t: &Theme, s| s.bg(ChartPalette::series(si, t.mode))),
                );

                if self.show_values && val.abs() > f32::EPSILON {
                    let label = fmt_tick(val, ax.step);
                    let label_top = (bar_top - 14.0).max(0.0);
                    plot = plot.child(
                        text(label)
                            .size(TextSize::Xs)
                            .tabular()
                            .absolute()
                            .top(label_top)
                            .left(sub_left)
                            .w(sub_bar_w)
                            .text_align(TextAlign::Center)
                            .themed(|t: &Theme, s| s.color(t.text_muted)),
                    );
                }
            }
        }

        plot = with_axis_titles(
            plot,
            &ax,
            plot_total_h,
            x_title.as_deref(),
            y_title.as_deref(),
        );

        col()
            .gap(0.0)
            .semantics(Semantics::Image)
            .label(desc)
            .children((plot, legend_row::<Msg>(&series_names)))
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fenestra_core::oklch_of;

    // ── Existing palette tests ────────────────────────────────────────────────

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

    // ── Tick math ─────────────────────────────────────────────────────────────

    #[test]
    fn nice_ticks_spans_range_and_covers_endpoints() {
        let t = nice_ticks(1.3, 9.7, 5);
        assert!(
            !t.is_empty(),
            "nice_ticks must return at least one tick for a valid range"
        );
        assert!(
            t.first().copied().unwrap_or(f32::INFINITY) <= 1.3,
            "first tick must be ≤ data lo"
        );
        assert!(
            t.last().copied().unwrap_or(f32::NEG_INFINITY) >= 9.7,
            "last tick must be ≥ data hi"
        );
    }

    #[test]
    fn nice_ticks_steps_are_125() {
        // The step between adjacent ticks must be a 1/2/5 × 10^k value.
        for (lo, hi) in [(0.0, 1.0), (0.0, 100.0), (-50.0, 50.0), (1234.0, 5678.0)] {
            let t = nice_ticks(lo, hi, 5);
            if t.len() >= 2 {
                let step = (t[1] - t[0]).abs();
                let ns = nice_step(step);
                assert!(
                    (step - ns).abs() < step * 1e-4,
                    "step {step} for ({lo},{hi}) is not a nice number"
                );
            }
        }
    }

    #[test]
    fn nice_ticks_flat_range_expands() {
        let t = nice_ticks(5.0, 5.0, 4);
        assert!(t.len() >= 2, "flat range must still yield ticks");
        assert!(t.first().copied().unwrap_or(5.0) < 5.0);
        assert!(t.last().copied().unwrap_or(5.0) > 5.0);
    }

    #[test]
    fn nice_ticks_hostile_inputs_do_not_panic() {
        // Non-finite inputs must not panic and must return something.
        let _ = nice_ticks(f32::NAN, f32::INFINITY, 5);
        let _ = nice_ticks(f32::NEG_INFINITY, f32::NAN, 0);
        let _ = nice_ticks(1.0, 0.0, 1); // reversed
    }

    #[test]
    fn fmt_tick_integer_step_no_decimals() {
        assert_eq!(fmt_tick(42.0, 10.0), "42");
        assert_eq!(fmt_tick(-5.0, 1.0), "-5");
        assert_eq!(fmt_tick(0.0, 5.0), "0");
    }

    #[test]
    fn fmt_tick_sub_integer_step_shows_decimals() {
        // nice_step only produces 1/2/5 × 10^k values; test those.
        assert_eq!(fmt_tick(1.5, 0.5), "1.5"); // 5×10^-1 → 1 decimal
        assert_eq!(fmt_tick(0.2, 0.2), "0.2"); // 2×10^-1 → 1 decimal
        assert_eq!(fmt_tick(0.05, 0.05), "0.05"); // 5×10^-2 → 2 decimals
    }

    #[test]
    fn fmt_tick_non_finite_returns_dash() {
        assert_eq!(fmt_tick(f32::NAN, 1.0), "–");
        assert_eq!(fmt_tick(f32::INFINITY, 1.0), "–");
    }

    // ── Path helpers ──────────────────────────────────────────────────────────

    #[test]
    fn pie_slice_zero_sweep_is_empty() {
        let bez = pie_slice(0.0, 0.0, 0.45);
        assert_eq!(bez.elements().len(), 0);
    }

    #[test]
    fn donut_slice_full_sweep_produces_elements() {
        let bez = donut_slice(-FRAC_PI_2, TAU, 0.47, 0.47 * 0.55);
        assert!(
            !bez.elements().is_empty(),
            "full-sweep donut must produce path elements"
        );
    }

    #[test]
    fn area_fill_path_shorter_than_two_is_empty() {
        assert_eq!(area_fill_path(&[]).elements().len(), 0);
        assert_eq!(area_fill_path(&[1.0]).elements().len(), 0);
    }

    #[test]
    fn area_fill_path_has_close() {
        let bez = area_fill_path(&[0.2, 0.8, 0.5]);
        assert!(
            bez.elements()
                .iter()
                .any(|e| matches!(e, kurbo::PathEl::ClosePath)),
            "area path must close"
        );
    }

    // ── Builder smoke tests ───────────────────────────────────────────────────

    #[test]
    fn line_chart_builder_empty_does_not_panic() {
        let _: Element<()> = LineChartBuilder::new(std::iter::empty::<f32>()).build();
    }

    #[test]
    fn line_chart_builder_with_markers_does_not_panic() {
        let _: Element<()> = LineChartBuilder::new([1.0, 3.0, 2.0, 5.0])
            .show_markers()
            .build();
    }

    #[test]
    fn bar_chart_axes_empty_does_not_panic() {
        let _: Element<()> = BarChartAxes::new(std::iter::empty::<(&str, f32)>()).build();
    }

    #[test]
    fn bar_chart_axes_with_values_does_not_panic() {
        let _: Element<()> = BarChartAxes::new([("a", 3.0), ("b", 7.0)])
            .show_values()
            .build();
    }

    #[test]
    fn multi_series_chart_empty_does_not_panic() {
        let _: Element<()> = MultiSeriesChart::new(std::iter::empty::<(&str, Vec<f32>)>()).build();
    }

    #[test]
    fn area_chart_empty_does_not_panic() {
        let _: Element<()> = area_chart(std::iter::empty::<f32>());
    }

    #[test]
    fn scatter_chart_empty_does_not_panic() {
        let _: Element<()> = ScatterChart::new(std::iter::empty::<(f32, f32)>()).build();
    }

    #[test]
    fn pie_chart_empty_does_not_panic() {
        let _: Element<()> = PieChart::new(std::iter::empty::<(&str, f32)>()).build();
    }

    #[test]
    fn pie_chart_single_segment_does_not_panic() {
        let _: Element<()> = PieChart::new([("only", 1.0_f32)]).build();
    }

    #[test]
    fn donut_chart_does_not_panic() {
        let _: Element<()> = PieChart::new([("a", 60.0_f32), ("b", 40.0)])
            .donut(0.55)
            .build();
    }

    #[test]
    fn stacked_bar_chart_empty_does_not_panic() {
        let _: Element<()> = StackedBarChart::new(
            std::iter::empty::<&str>(),
            std::iter::empty::<(&str, Vec<f32>)>(),
        )
        .build();
    }

    #[test]
    fn grouped_bar_chart_does_not_panic() {
        let _: Element<()> = GroupedBarChart::new(
            ["Q1", "Q2", "Q3"],
            [
                ("a", vec![1.0_f32, 2.0, 3.0]),
                ("b", vec![3.0_f32, 2.0, 1.0]),
            ],
        )
        .show_values()
        .build();
    }

    #[test]
    fn hostile_data_builders_do_not_panic() {
        let _ = LineChartBuilder::new([f32::NAN, f32::INFINITY, -1.0, 1.0])
            .show_markers()
            .build::<()>();
        let _ =
            BarChartAxes::new([("neg", -5.0_f32), ("nan", f32::NAN), ("ok", 3.0)]).build::<()>();
        let _ =
            ScatterChart::new([(f32::NAN, 1.0), (1.0, f32::INFINITY), (2.0, 3.0)]).build::<()>();
        let _ = PieChart::new([("zero", 0.0_f32), ("nan", f32::NAN), ("ok", 5.0)]).build::<()>();
        let _ = StackedBarChart::new(["x"], [("s", vec![f32::NAN, f32::INFINITY])]).build::<()>();
        let _ = GroupedBarChart::new(["x"], [("s", vec![f32::NAN])]).build::<()>();
    }

    // ── Accessible descriptions: an agent reads these from the access tree ─────

    /// Every accessible (`Semantics::Image`) label projected by a chart, so a
    /// test can assert the agent-visible description carries the right facts —
    /// exactly what the headless verification story depends on.
    fn image_labels(el: Element<()>, size: (f32, f32)) -> String {
        use fenestra_core::{Fonts, FrameState, build_frame, by};
        let mut fonts = Fonts::embedded();
        let mut state = FrameState::new();
        let frame = build_frame(&el, &Theme::light(), &mut fonts, &mut state, size, 1.0);
        frame
            .get_all(&by::role(Semantics::Image))
            .into_iter()
            .filter_map(|n| n.label)
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn assert_has(haystack: &str, needles: &[&str]) {
        for n in needles {
            assert!(
                haystack.contains(n),
                "expected {n:?} in label(s): {haystack:?}"
            );
        }
    }

    #[test]
    fn sparkline_label_carries_data() {
        let l = image_labels(sparkline([3.0_f32, 5.0, 2.0, 8.0, 9.0]), (200.0, 80.0));
        assert_has(&l, &["sparkline", "5 points", "min 2", "max 9", "rising"]);
    }

    #[test]
    fn line_chart_label_carries_data() {
        let l = image_labels(line_chart([10.0_f32, 8.0, 6.0, 2.0]), (320.0, 160.0));
        assert_has(
            &l,
            &["line chart", "4 points", "min 2", "max 10", "falling"],
        );
    }

    #[test]
    fn bar_chart_label_carries_count_and_range() {
        let l = image_labels(
            bar_chart([("mon", 4.0), ("tue", 7.0), ("wed", 3.0)]),
            (320.0, 160.0),
        );
        assert_has(&l, &["bar chart", "3 bars", "min 3", "max 7", "mon", "tue"]);
    }

    #[test]
    fn multi_line_chart_label_carries_series_and_range() {
        let l = image_labels(
            multi_line_chart(vec![
                ("a".to_string(), vec![1.0_f32, 4.0, 2.0]),
                ("b".to_string(), vec![3.0_f32, 6.0, 5.0]),
            ]),
            (320.0, 160.0),
        );
        assert_has(&l, &["2 series", "3 points each", "min 1", "max 6"]);
    }

    #[test]
    fn line_chart_builder_label_carries_data() {
        let l = image_labels(
            LineChartBuilder::new([5.0_f32, 5.0, 5.0, 5.0]).build(),
            (320.0, 160.0),
        );
        assert_has(&l, &["line chart", "4 points", "min 5", "max 5", "flat"]);
    }

    #[test]
    fn bar_chart_axes_label_carries_data() {
        let l = image_labels(
            BarChartAxes::new([("Mon", 4.0_f32), ("Tue", 7.0)]).build(),
            (320.0, 160.0),
        );
        assert_has(&l, &["bar chart", "2 bars", "min 4", "max 7"]);
    }

    #[test]
    fn multi_series_chart_label_carries_data() {
        let l = image_labels(
            MultiSeriesChart::new([
                ("cpu", vec![12.0_f32, 18.0, 9.0]),
                ("mem", vec![20.0_f32, 16.0, 22.0]),
            ])
            .build(),
            (320.0, 220.0),
        );
        assert_has(&l, &["2 series", "3 points each", "min 9", "max 22"]);
    }

    #[test]
    fn area_chart_label_carries_data() {
        let l = image_labels(area_chart([2.0_f32, 6.0, 4.0, 9.0]), (320.0, 160.0));
        assert_has(&l, &["area chart", "4 points", "min 2", "max 9", "rising"]);
    }

    #[test]
    fn scatter_chart_label_carries_ranges() {
        let l = image_labels(
            ScatterChart::new([(1.0_f32, 2.0_f32), (3.0, 8.0), (5.0, 4.0)]).build(),
            (320.0, 240.0),
        );
        assert_has(&l, &["scatter chart", "3 points", "x 1", "y 2", "8"]);
    }

    #[test]
    fn pie_chart_label_enumerates_slices() {
        let l = image_labels(
            PieChart::new([("Alpha", 50.0_f32), ("Beta", 30.0), ("Gamma", 20.0)]).build(),
            (200.0, 240.0),
        );
        assert_has(&l, &["pie chart", "3 slices", "Alpha 50%", "Beta 30%"]);
    }

    #[test]
    fn donut_chart_label_enumerates_slices() {
        let l = image_labels(
            PieChart::new([("A", 60.0_f32), ("B", 40.0)])
                .donut(0.55)
                .build(),
            (200.0, 240.0),
        );
        assert_has(&l, &["donut chart", "2 slices", "A 60%", "B 40%"]);
    }

    #[test]
    fn stacked_bar_chart_label_carries_grid() {
        let l = image_labels(
            StackedBarChart::new(
                ["Mon", "Tue", "Wed"],
                [
                    ("web", vec![3.0_f32, 5.0, 4.0]),
                    ("api", vec![2.0_f32, 3.0, 6.0]),
                ],
            )
            .build(),
            (320.0, 200.0),
        );
        assert_has(&l, &["stacked bar chart", "2 series", "3 groups"]);
    }

    #[test]
    fn grouped_bar_chart_label_carries_grid() {
        let l = image_labels(
            GroupedBarChart::new(
                ["Q1", "Q2"],
                [
                    ("a", vec![3.0_f32, 5.0]),
                    ("b", vec![2.0_f32, 4.0]),
                    ("c", vec![1.0_f32, 2.0]),
                ],
            )
            .build(),
            (320.0, 200.0),
        );
        assert_has(&l, &["grouped bar chart", "3 series", "2 groups"]);
    }

    #[test]
    fn empty_charts_report_no_data() {
        let l = image_labels(line_chart(std::iter::empty::<f32>()), (320.0, 160.0));
        assert_has(&l, &["line chart", "no data"]);
        let l = image_labels(
            PieChart::new(std::iter::empty::<(&str, f32)>()).build(),
            (200.0, 200.0),
        );
        assert_has(&l, &["pie chart", "no data"]);
    }

    #[test]
    fn axis_titles_appear_in_label() {
        let l = image_labels(
            LineChartBuilder::new([1.0_f32, 2.0, 3.0])
                .x_title("Day")
                .y_title("Requests")
                .build(),
            (320.0, 160.0),
        );
        assert_has(&l, &["x-axis: Day", "y-axis: Requests"]);
    }

    // ── Description-helper unit tests (deterministic, no rendering) ────────────

    #[test]
    fn fmt_num_is_compact_and_deterministic() {
        assert_eq!(fmt_num(12.0), "12");
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(-5.0), "-5");
        assert_eq!(fmt_num(0.0), "0");
        assert_eq!(fmt_num(-0.0), "0");
        assert_eq!(fmt_num(1234.0), "1234");
        assert_eq!(fmt_num(f32::NAN), "n/a");
    }

    #[test]
    fn count_noun_pluralizes() {
        assert_eq!(count_noun(1, "point"), "1 point");
        assert_eq!(count_noun(0, "point"), "0 points");
        assert_eq!(count_noun(7, "bar"), "7 bars");
    }

    #[test]
    fn trend_word_classifies_direction() {
        assert_eq!(trend_word(&[1.0, 2.0, 3.0]), "rising");
        assert_eq!(trend_word(&[3.0, 2.0, 1.0]), "falling");
        assert_eq!(trend_word(&[5.0, 5.0, 5.0]), "flat");
        assert_eq!(trend_word(&[]), "flat");
    }

    #[test]
    fn bounded_labels_caps_and_counts_remainder() {
        let v: Vec<String> = ["a", "b", "c", "d", "e", "f"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(bounded_labels(&v, 4), "a, b, c, d, +2 more");
        assert_eq!(bounded_labels(&v[..2], 4), "a, b");
    }
}
