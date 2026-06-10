//! Display widgets: Card, StatCard, Badge, Avatar, Progress, Spinner,
//! Callout, Tabs, and Table.

use fenestra_core::{
    Element, Length, MotionDuration, R_FULL, R_LG, R_MD, SP1, SP2, SP3, SP4, SP6, ShadowToken,
    StatusColors, TextSize, Theme, Track, Transition, Weight, col, div, path, row, text,
};
use kurbo::BezPath;

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
    col()
        .p(SP6)
        .gap(SP3)
        .rounded(R_LG)
        .shadow(ShadowToken::Sm)
        .themed(|t: &Theme, s| s.bg(t.surface_raised).border(1.0, t.border_subtle))
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
    fn colors(self, t: &Theme) -> StatusColors {
        match self {
            Self::Accent => StatusColors {
                bg: t.accent_bg,
                border: t.accent_border,
                solid: t.accent,
                solid_hover: t.accent_hover,
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
        let mut value_row = row()
            .items_baseline()
            .gap(SP2)
            .children([text(s.value).size(TextSize::Xl2).weight(Weight::Semibold)]);
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
        .rounded(R_MD)
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
    on_select: impl Fn(usize) -> Msg,
) -> Element<Msg> {
    let labels: Vec<String> = labels.into_iter().map(Into::into).collect();
    row()
        .gap(SP4)
        .themed(|t: &Theme, s| s.border(0.0, t.border_subtle))
        .children(labels.into_iter().enumerate().map(|(i, label)| {
            let is_active = i == active;
            col()
                .items_center()
                .gap(SP1)
                .pt(SP1)
                .focusable(true)
                .cursor(fenestra_core::Cursor::Pointer)
                .on_click(on_select(i))
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
        }))
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
                .hover_themed(|t, s| s.bg(t.surface))
                .children(
                    cells
                        .into_iter()
                        .map(|c| text(c).size(TextSize::Sm).truncate()),
                )
        }))
}
