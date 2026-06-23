//! Display widgets: Card, StatCard, Badge, Avatar, Progress, Spinner,
//! Callout, Tabs, and Table.

use fenestra_core::{
    CubicBezier, Element, GridTemplate, Key, Keyframes, Length, MEASURE_CH, MotionDuration, R_FULL,
    SP1, SP2, SP3, SP4, SP6, Semantics, StatusColors, Surface, TextSize, Theme, Track, Transition,
    Weight, col, div, path, row, stack, text,
};
use kurbo::BezPath;

/// A vertical prose column pre-capped at the default reading measure
/// ([`MEASURE_CH`]). Children stack with no gap; add text and the column holds
/// about 66 characters per line regardless of window width. Set `.size(..)` to
/// match the prose so the measure tracks the real glyph width (1ch is the
/// advance of `'0'` in the column's own resolved text style).
///
/// ```
/// use fenestra_core::text;
/// use fenestra_kit::reading_column;
///
/// let el: fenestra_core::Element<()> = reading_column().children([text("Body")]);
/// ```
pub fn reading_column<Msg>() -> Element<Msg> {
    col().measure(MEASURE_CH)
}

/// The signature card: raised surface, subtle border, large radius, small
/// shadow, SP6 padding.
///
/// ```
/// use fenestra_core::text;
/// use fenestra_kit::card;
///
/// let el: fenestra_core::Element<()> = card().children([text("Content")]);
/// ```
pub fn card<Msg>() -> Element<Msg> {
    col().p(SP6).gap(SP3).surface(Surface::Card)
}

/// Which status palette a badge or callout uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    /// Accent-tinted (informational).
    #[default]
    Accent,
    /// Red: errors and destructive outcomes.
    Danger,
    /// Amber: caution.
    Warning,
    /// Green: success.
    Success,
}

impl Status {
    pub(crate) fn colors(self, t: &Theme) -> StatusColors {
        match self {
            Self::Accent => StatusColors {
                bg: t.accent_bg,
                border: t.accent_border,
                solid: t.accent,
                solid_hover: t.accent_hover,
                solid_active: t.accent_active,
                text: t.accent_text,
            },
            Self::Danger => t.danger,
            Self::Warning => t.warning,
            Self::Success => t.success,
        }
    }
}

/// A small status pill: tinted background with darker text.
///
/// ```
/// use fenestra_kit::{Status, badge};
///
/// let el: fenestra_core::Element<()> = badge("+12%", Status::Success);
/// ```
pub fn badge<Msg>(label: impl Into<String>, status: Status) -> Element<Msg> {
    row()
        .items_center()
        .px(SP2)
        .py(2.0)
        .rounded_full()
        .shrink0()
        .themed(move |t: &Theme, s| s.bg(status.colors(t).bg))
        .children([text(label)
            .size(TextSize::Xs)
            .weight(Weight::Medium)
            .themed(move |t: &Theme, s| s.color(status.colors(t).text))])
}

/// A circular initials avatar in the accent tint.
pub fn avatar<Msg>(initials: impl Into<String>) -> Element<Msg> {
    row()
        .items_center()
        .justify_center()
        .w(32.0)
        .h(32.0)
        .rounded_full()
        .shrink0()
        .themed(|t: &Theme, s| s.bg(t.accent_bg))
        .children([text(initials)
            .size(TextSize::Xs)
            .weight(Weight::Medium)
            .themed(|t: &Theme, s| s.color(t.accent_text))])
}

/// A metric card: muted label, large value, optional delta badge.
///
/// ```
/// use fenestra_kit::{Status, stat_card};
///
/// let el: fenestra_core::Element<()> =
///     stat_card("Revenue", "$48,210").delta("+12.5%", Status::Success).into();
/// ```
pub fn stat_card<Msg>(label: impl Into<String>, value: impl Into<String>) -> StatCard<Msg> {
    StatCard {
        label: label.into(),
        value: value.into(),
        delta: None,
        _msg: std::marker::PhantomData,
    }
}

/// A stat card under construction; converts into an [`Element`].
pub struct StatCard<Msg> {
    label: String,
    value: String,
    delta: Option<(String, Status)>,
    _msg: std::marker::PhantomData<Msg>,
}

impl<Msg> StatCard<Msg> {
    /// Adds a delta badge next to the value.
    pub fn delta(mut self, label: impl Into<String>, status: Status) -> Self {
        self.delta = Some((label.into(), status));
        self
    }
}

impl<Msg> From<StatCard<Msg>> for Element<Msg> {
    fn from(s: StatCard<Msg>) -> Self {
        let mut value_row = row().items_baseline().gap(SP2).children([text(s.value)
            .size(TextSize::Xl2)
            .weight(Weight::Semibold)
            .tabular()]);
        if let Some((delta, status)) = s.delta {
            value_row = value_row.child(badge(delta, status));
        }
        card().gap(SP1).children([
            text(s.label)
                .size(TextSize::Sm)
                .weight(Weight::Medium)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
            value_row,
        ])
    }
}

/// A status dot without a label (presence/health indicators). Pair
/// with text or a tooltip for context.
pub fn badge_dot<Msg>(status: Status) -> Element<Msg> {
    div()
        .w(8.0)
        .h(8.0)
        .rounded(R_FULL)
        .shrink0()
        .themed(move |t: &Theme, s| s.bg(status.colors(t).solid))
        .semantics(Semantics::Image)
        .label(format!("{status:?} indicator"))
}

/// A labeled status indicator: a semantic dot plus a text label. Call
/// [`StatusIndicator::live`] for a pulsing "sonar" ring — the realtime /
/// online / recording cue. The dot is decorative; the label carries the
/// meaning, so a screen reader reads the words, not a colored circle.
///
/// ```
/// use fenestra_kit::{Status, status};
///
/// let el: fenestra_core::Element<()> = status("Operational", Status::Success).into();
/// ```
pub fn status<Msg>(label: impl Into<String>, status: Status) -> StatusIndicator<Msg> {
    StatusIndicator {
        label: label.into(),
        status,
        live: false,
        _msg: std::marker::PhantomData,
    }
}

/// A status indicator under construction; converts into an [`Element`].
pub struct StatusIndicator<Msg> {
    label: String,
    status: Status,
    live: bool,
    _msg: std::marker::PhantomData<Msg>,
}

impl<Msg> StatusIndicator<Msg> {
    /// Adds a pulsing sonar ring for realtime states (online / recording /
    /// live). Static under reduced motion, so headless renders stay
    /// deterministic — the ring only animates in a live window.
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }
}

impl<Msg> From<StatusIndicator<Msg>> for Element<Msg> {
    fn from(s: StatusIndicator<Msg>) -> Self {
        let status = s.status;
        let indicator: Element<Msg> = if s.live {
            // A sonar ring blooms out from under the dot and fades (fast bloom,
            // slow fade per the easing); the opaque dot rides on top.
            let ring = div::<Msg>()
                .w(8.0)
                .h(8.0)
                .rounded(R_FULL)
                .themed(move |t: &Theme, st| st.bg(status.colors(t).solid))
                .keyframes(
                    Keyframes::new(1600.0)
                        .ease(CubicBezier {
                            x1: 0.22,
                            y1: 0.61,
                            x2: 0.36,
                            y2: 1.0,
                        })
                        .stop(0.0, |st| st.opacity(0.30).scale(1.7))
                        .stop(0.7, |st| st.opacity(0.12).scale(2.6))
                        .stop(1.0, |st| st.opacity(0.0).scale(3.0)),
                );
            let dot = div::<Msg>()
                .w(8.0)
                .h(8.0)
                .rounded(R_FULL)
                .themed(move |t: &Theme, st| st.bg(status.colors(t).solid));
            stack().w(8.0).h(8.0).shrink0().children([ring, dot])
        } else {
            div::<Msg>()
                .w(8.0)
                .h(8.0)
                .rounded(R_FULL)
                .shrink0()
                .themed(move |t: &Theme, st| st.bg(status.colors(t).solid))
        };
        row()
            .items_center()
            .gap(SP2)
            .shrink0()
            .children([indicator])
            .children([text(s.label)
                .size(TextSize::Sm)
                .themed(|t: &Theme, st| st.color(t.text_muted))])
    }
}

/// An indeterminate activity bar: the fill sweeps from empty to full
/// and fades, looping. Use when progress has no known fraction; pinned
/// at the first keyframe under reduced motion.
pub fn progress_indeterminate<Msg>() -> Element<Msg> {
    div()
        .w_full()
        .h(4.0)
        .rounded(R_FULL)
        .overflow_hidden()
        .themed(|t: &Theme, s| s.bg(t.neutrals.step(4)))
        .children([div()
            .h_full()
            .rounded(R_FULL)
            .themed(|t: &Theme, s| s.bg(t.accent))
            .keyframes(
                Keyframes::new(1200.0)
                    .stop(0.0, |s| s.w(Length::Pct(6.0)).opacity(1.0))
                    .stop(0.7, |s| s.w(Length::Pct(100.0)).opacity(1.0))
                    .stop(1.0, |s| s.w(Length::Pct(100.0)).opacity(0.0)),
            )])
}

/// A 4px progress bar; the fill animates toward the new fraction.
pub fn progress<Msg>(fraction: f32) -> Element<Msg> {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .w_full()
        .h(4.0)
        .rounded(R_FULL)
        .themed(|t: &Theme, s| s.bg(t.neutrals.step(4)))
        .children([div()
            .id("fill")
            .h_full()
            .rounded(R_FULL)
            .w(Length::Pct(fraction * 100.0))
            .transition(
                Transition::colors()
                    .lengths(true)
                    .duration(MotionDuration::Base),
            )
            .themed(|t: &Theme, s| s.bg(t.accent))])
}

/// Which semantic zone a [`meter`] value falls in, per the HTML `<meter>` model.
#[derive(Clone, Copy)]
enum MeterZone {
    Good,
    Suboptimal,
    Poor,
}

/// A meter under construction; converts into an [`Element`].
pub struct Meter {
    value: f32,
    min: f32,
    max: f32,
    low: Option<f32>,
    high: Option<f32>,
    optimum: Option<f32>,
    label: Option<String>,
}

/// A scalar measurement within a known `min..=max` range — disk usage, a score,
/// signal strength — distinct from [`progress`], which is task completion. Drawn
/// as a filled bar; set any of [`Meter::low`] / [`Meter::high`] /
/// [`Meter::optimum`] and the fill colours by the HTML `<meter>` zone model
/// (success / warning / danger), otherwise it rests on the accent.
pub fn meter(value: f32, min: f32, max: f32) -> Meter {
    Meter {
        value,
        min,
        max,
        low: None,
        high: None,
        optimum: None,
        label: None,
    }
}

impl Meter {
    /// The low-end threshold (defaults to `min`).
    #[must_use]
    pub fn low(mut self, low: f32) -> Self {
        self.low = Some(low);
        self
    }

    /// The high-end threshold (defaults to `max`).
    #[must_use]
    pub fn high(mut self, high: f32) -> Self {
        self.high = Some(high);
        self
    }

    /// The optimum end that marks "good" (defaults to `max` — higher is better).
    /// Set it at or below `low` for lower-is-better measurements.
    #[must_use]
    pub fn optimum(mut self, optimum: f32) -> Self {
        self.optimum = Some(optimum);
        self
    }

    /// A caption shown above the bar, paired with the value as a percentage.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The colour zone for the current value, or `None` when no thresholds are
    /// set (a neutral, accent-filled measurement).
    fn zone(&self) -> Option<MeterZone> {
        if self.low.is_none() && self.high.is_none() && self.optimum.is_none() {
            return None;
        }
        let low = self.low.unwrap_or(self.min);
        let high = self.high.unwrap_or(self.max);
        let optimum = self.optimum.unwrap_or(self.max);
        let v = self.value;
        let zone = if optimum <= low {
            // Lower is better.
            if v <= low {
                MeterZone::Good
            } else if v <= high {
                MeterZone::Suboptimal
            } else {
                MeterZone::Poor
            }
        } else if optimum >= high {
            // Higher is better.
            if v >= high {
                MeterZone::Good
            } else if v >= low {
                MeterZone::Suboptimal
            } else {
                MeterZone::Poor
            }
        } else if v >= low && v <= high {
            // A middle band is best.
            MeterZone::Good
        } else {
            MeterZone::Suboptimal
        };
        Some(zone)
    }
}

impl<Msg> From<Meter> for Element<Msg> {
    fn from(m: Meter) -> Self {
        let span = (m.max - m.min).abs().max(f32::EPSILON);
        let frac = ((m.value - m.min) / span).clamp(0.0, 1.0);
        let zone = m.zone();

        let fill = div()
            .h_full()
            .rounded(R_FULL)
            .w(Length::Pct(frac * 100.0))
            .transition(
                Transition::colors()
                    .lengths(true)
                    .duration(MotionDuration::Base),
            )
            .themed(move |t: &Theme, s| {
                let c = match zone {
                    None => t.accent,
                    Some(MeterZone::Good) => t.success.solid,
                    Some(MeterZone::Suboptimal) => t.warning.solid,
                    Some(MeterZone::Poor) => t.danger.solid,
                };
                s.bg(c)
            });
        let track = div()
            .w_full()
            .h(8.0)
            .rounded(R_FULL)
            .themed(|t: &Theme, s| s.bg(t.neutrals.step(4)))
            .children([fill]);

        match m.label {
            Some(label) => {
                let caption = row().items_center().justify_between().children([
                    text(label)
                        .size(TextSize::Sm)
                        .themed(|t: &Theme, s| s.color(t.text)),
                    text(format!("{:.0}%", frac * 100.0))
                        .size(TextSize::Sm)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                ]);
                col().w_full().gap(SP1).children([caption, track])
            }
            None => track,
        }
    }
}

/// A determinate progress bar drawn the Material 3 Expressive way: the filled
/// portion is an accent **sine wave** that flattens into the track at its
/// leading edge and as it nears completion, separated from the remaining
/// neutral track by a small gap. The wave is static, so headless renders are
/// deterministic; for an indeterminate sweep use [`progress_indeterminate`].
///
/// ```
/// use fenestra_kit::wavy_progress;
///
/// let el: fenestra_core::Element<()> = wavy_progress(0.6, 240.0).into();
/// ```
pub fn wavy_progress<Msg>(fraction: f32, width: f32) -> WavyProgress<Msg> {
    WavyProgress {
        fraction,
        width,
        amplitude: 2.5,
        wavelength: 16.0,
        _msg: std::marker::PhantomData,
    }
}

/// A wavy progress bar under construction; converts into an [`Element`].
pub struct WavyProgress<Msg> {
    fraction: f32,
    width: f32,
    amplitude: f32,
    wavelength: f32,
    _msg: std::marker::PhantomData<Msg>,
}

impl<Msg> WavyProgress<Msg> {
    /// Sets the wave amplitude (peak deflection from the centerline, px).
    pub fn amplitude(mut self, amplitude: f32) -> Self {
        self.amplitude = amplitude.max(0.0);
        self
    }

    /// Sets the wavelength (logical px per cycle).
    pub fn wavelength(mut self, wavelength: f32) -> Self {
        self.wavelength = wavelength.max(4.0);
        self
    }
}

impl<Msg> From<WavyProgress<Msg>> for Element<Msg> {
    fn from(p: WavyProgress<Msg>) -> Self {
        let fraction = p.fraction.clamp(0.0, 1.0);
        let width = p.width.max(1.0);
        let amp = f64::from(p.amplitude);
        let wavelength = f64::from(p.wavelength);
        let stroke = 3.0_f64;
        let gap = 5.0_f64;
        let w = f64::from(width);
        let h = amp * 2.0 + stroke + 2.0;
        let cy = h / 2.0;
        let h_px = h as f32;
        let active_w = f64::from(fraction) * w;
        // Flatten the whole wave over the final stretch so a near-complete bar
        // reads as a smooth line (M3's amplitude-to-zero at completion).
        let completion = ((1.0 - f64::from(fraction)) / 0.12).clamp(0.0, 1.0);

        // The active wave, sampled as a dense polyline (round joins smooth it).
        // On a long enough bar the amplitude tapers to zero over the last
        // wavelength so the wave eases into the gap rather than stopping
        // mid-swing; short bars keep full amplitude.
        let mut wave = BezPath::new();
        if active_w > 0.5 {
            wave.move_to((0.0, cy));
            let mut x = 0.0_f64;
            while x < active_w {
                x = (x + 1.5).min(active_w);
                let edge = if active_w > 2.0 * wavelength {
                    ((active_w - x) / wavelength).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let a = amp * edge * completion;
                let y = cy + a * (std::f64::consts::TAU * x / wavelength).sin();
                wave.line_to((x, y));
            }
        }

        // The remaining track: a flat line after the gap (full width at 0%).
        let track_start = if active_w > 0.5 {
            (active_w + gap).min(w)
        } else {
            0.0
        };
        let mut track = BezPath::new();
        track.move_to((track_start, cy));
        track.line_to((w, cy));

        stack().w(width).h(h_px).children([
            path(track, (w, h), Some(stroke)).themed(|t: &Theme, s| s.color(t.neutrals.step(4))),
            path(wave, (w, h), Some(stroke)).themed(|t: &Theme, s| s.color(t.accent)),
        ])
    }
}

/// A rotating arc spinner (800ms per turn; static under reduced motion).
pub fn spinner<Msg>() -> Element<Msg> {
    // A 270-degree arc in a 16x16 viewbox, drawn from four quarter-turn
    // cubic segments (kappa circle approximation).
    let (cx, cy, r) = (8.0, 8.0, 6.0);
    let k = 0.552_284_749_8 * r;
    let mut p = BezPath::new();
    p.move_to((cx + r, cy));
    p.curve_to((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r));
    p.curve_to((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy));
    p.curve_to((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r));
    path(p, (16.0, 16.0), Some(2.0))
        .spin(800.0)
        .themed(|t: &Theme, s| s.color(t.accent))
}

/// A status callout: tinted background, status border, icon, and message.
///
/// ```
/// use fenestra_kit::{Status, callout};
///
/// let el: fenestra_core::Element<()> =
///     callout(Status::Warning, "Your trial ends in 3 days.");
/// ```
pub fn callout<Msg>(status: Status, message: impl Into<String>) -> Element<Msg> {
    let icon =
        crate::icons::circle_dot().themed(move |t: &Theme, s| s.color(status.colors(t).text));
    // Center the icon against the first line of Sm text (21px line box).
    let icon = row().h(21.0).items_center().shrink0().children([icon]);
    row()
        .items_start()
        .gap(SP2)
        .p(SP4)
        .themed(|t: &Theme, s| s.rounded(t.radius.md))
        .themed(move |t: &Theme, s| {
            let c = status.colors(t);
            s.bg(c.bg).border(1.0, c.border)
        })
        .children([icon])
        .children([text(message)
            .size(TextSize::Sm)
            .themed(move |t: &Theme, s| s.color(status.colors(t).text))
            .grow()])
}

/// Underline tabs: the active tab's 2px accent indicator cross-fades 200ms.
///
/// ```
/// use fenestra_kit::tabs;
///
/// #[derive(Clone)]
/// enum Msg {
///     Tab(usize),
/// }
///
/// let el: fenestra_core::Element<Msg> =
///     tabs(0, ["Overview", "Activity", "Settings"], Msg::Tab);
/// ```
pub fn tabs<Msg: Clone + 'static>(
    active: usize,
    labels: impl IntoIterator<Item = impl Into<String>>,
    on_select: impl Fn(usize) -> Msg + 'static,
) -> Element<Msg> {
    let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
    let n = labels.len();
    let active = if n == 0 { 0 } else { active.min(n - 1) };
    // Shared between the per-tab click handlers and the strip-level key handler.
    let on_select = std::rc::Rc::new(on_select);
    let tab_select = on_select.clone();
    let strip = row()
        .gap(SP4)
        .themed(|t: &Theme, s| s.border(0.0, t.border_subtle))
        .children(labels.into_iter().enumerate().map(move |(i, label)| {
            let is_active = i == active;
            col()
                .items_center()
                .gap(SP1)
                .pt(SP1)
                .cursor(fenestra_core::Cursor::Pointer)
                .on_click(tab_select(i))
                // One tab stop for the strip (see the strip's key handler);
                // `on_click` auto-focuses, so opt the tabs back out.
                .focusable(false)
                .semantics(Semantics::Tab {
                    selected: is_active,
                })
                .label(label.clone())
                .children([
                    text(label)
                        .size(TextSize::Sm)
                        .weight(Weight::Medium)
                        .transition(Transition::colors().duration(MotionDuration::Base))
                        .themed(move |t: &Theme, s| {
                            if is_active {
                                s.color(t.text)
                            } else {
                                s.color(t.text_muted)
                            }
                        }),
                    div()
                        .w_full()
                        .h(2.0)
                        .rounded(R_FULL)
                        .transition(Transition::colors().duration(MotionDuration::Base))
                        .themed(move |t: &Theme, s| {
                            if is_active {
                                s.bg(t.accent)
                            } else {
                                s.bg(t.accent.with_alpha(0.0))
                            }
                        }),
                ])
        }));
    // The strip is one tab stop; ←/→ (and ↑/↓) move + activate, Home/End jump.
    if n > 1 {
        strip.focusable(true).on_key(move |k| match k.key {
            Key::ArrowRight | Key::ArrowDown => (active + 1 < n).then(|| on_select(active + 1)),
            Key::ArrowLeft | Key::ArrowUp => (active > 0).then(|| on_select(active - 1)),
            Key::Home => Some(on_select(0)),
            Key::End => Some(on_select(n - 1)),
            _ => None,
        })
    } else {
        strip
    }
}

/// A responsive grid: as many equal columns as fit at `min_col` logical pixels
/// each, every child filling its track. Built on `repeat(auto-fit, minmax(min_col,
/// 1fr))`, so the column count adapts to the available width with no breakpoints —
/// the canonical responsive card layout. Children are gapped by [`SP4`].
///
/// ```
/// use fenestra_kit::{card, responsive_grid};
///
/// let grid: fenestra_core::Element<()> = responsive_grid(240.0, (0..6).map(|_| card()));
/// ```
pub fn responsive_grid<Msg>(
    min_col: f32,
    children: impl IntoIterator<Item = Element<Msg>>,
) -> Element<Msg> {
    div()
        .grid_cols([GridTemplate::auto_fit_minmax(min_col)])
        .gap(SP4)
        .w_full()
        .children(children)
}

/// A simple data table: muted header with a bottom rule, 44px rows with a
/// hover tint, equal fractional columns.
///
/// ```
/// use fenestra_kit::table;
///
/// let el: fenestra_core::Element<()> = table(
///     ["Name", "Status"],
///     vec![vec!["api-server".into(), "Healthy".into()]],
/// );
/// ```
pub fn table<Msg>(
    columns: impl IntoIterator<Item = impl Into<String>>,
    rows: Vec<Vec<String>>,
) -> Element<Msg> {
    let columns: Vec<String> = columns.into_iter().map(Into::into).collect();
    let n = columns.len().max(1);
    let tracks = || vec![Track::Fr(1.0); n];

    let header =
        div()
            .grid_cols(tracks())
            .gap(SP3)
            .pb(SP2)
            .px(SP2)
            .children(columns.into_iter().map(|c| {
                text(c)
                    .size(TextSize::Sm)
                    .weight(Weight::Medium)
                    .themed(|t: &Theme, s| s.color(t.text_muted))
            }));

    col()
        .w_full()
        .children([header, fenestra_core::divider()])
        .children(rows.into_iter().map(move |cells| {
            div()
                .grid_cols(tracks())
                .gap(SP3)
                .px(SP2)
                .h(44.0)
                .items_center()
                .shrink0()
                .transition(Transition::colors())
                .state_layer(|t| t.text)
                .children(
                    cells
                        .into_iter()
                        .map(|c| text(c).size(TextSize::Sm).truncate().tabular()),
                )
        }))
}
