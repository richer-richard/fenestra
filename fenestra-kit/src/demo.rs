//! M3 demo layouts: the holy-grail app shell (grid + absolute positioning)
//! and a nested scrolling layout. Shared by examples and golden tests.

use fenestra_core::{
    ChromeElevation, ChromeText, Element, R_LG, R_SM, SP1, SP2, SP3, SP4, ShadowToken, TextSize,
    Theme, Track, Weight, col, div, divider, row, text,
};

/// The holy-grail layout: header and footer spanning three grid columns,
/// sidebars of fixed width, fluid main content, plus an absolutely
/// positioned notification dot pinned to the header avatar.
pub fn holy_grail<Msg>(theme: &Theme) -> Element<Msg> {
    let header = row()
        .grid_row(1, 1)
        .grid_col(1, 3)
        .items_center()
        .justify_between()
        .px(SP4)
        .bg(theme.surface_raised)
        .border(1.0, theme.border_subtle)
        .children([
            text("fenestra").weight(Weight::Semibold),
            div()
                .w(28.0)
                .h(28.0)
                .rounded_full()
                .bg(theme.accent_bg)
                .children([div()
                    .absolute()
                    .top(-2.0)
                    .right(-2.0)
                    .w(10.0)
                    .h(10.0)
                    .rounded_full()
                    .bg(theme.danger.solid)
                    .border(2.0, theme.surface_raised)]),
        ]);

    let nav_item = |label: &str, active: bool| {
        let item = row()
            .items_center()
            .px(SP3)
            .h(32.0)
            .rounded(R_SM)
            .children([text(label).size(TextSize::Sm).color(if active {
                theme.accent_text
            } else {
                theme.text_muted
            })]);
        if active {
            item.bg(theme.accent_bg)
        } else {
            item
        }
    };

    let sidebar = col()
        .grid_row(2, 1)
        .grid_col(1, 1)
        .p(SP2)
        .gap(SP1)
        .bg(theme.surface)
        .children([
            nav_item("Overview", true),
            nav_item("Reports", false),
            nav_item("Settings", false),
        ]);

    let main = col()
        .grid_row(2, 1)
        .grid_col(2, 1)
        .p(SP4)
        .gap(SP3)
        .children([
            text("Main content")
                .size(TextSize::Lg)
                .weight(Weight::Semibold),
            text("The center column is fluid (1fr); sidebars are fixed.")
                .size(TextSize::Sm)
                .color(theme.text_muted),
            divider(),
            div()
                .h(96.0)
                .rounded(R_LG)
                .bg(theme.surface_raised)
                .border(1.0, theme.border_subtle)
                .shadow(ShadowToken::Sm),
        ]);

    let aside = col()
        .grid_row(2, 1)
        .grid_col(3, 1)
        .p(SP3)
        .gap(SP2)
        .bg(theme.surface)
        .children([
            text("Aside").size(TextSize::Sm).weight(Weight::Medium),
            text("Fixed 150px column.")
                .size(TextSize::Xs)
                .color(theme.text_subtle),
        ]);

    let footer = row()
        .grid_row(3, 1)
        .grid_col(1, 3)
        .items_center()
        .px(SP4)
        .bg(theme.surface)
        .children([text("footer — grid row 3")
            .size(TextSize::Xs)
            .color(theme.text_subtle)]);

    div()
        .bg(theme.bg)
        .grid_cols([Track::Px(180.0), Track::Fr(1.0), Track::Px(150.0)])
        .grid_rows([Track::Px(48.0), Track::Fr(1.0), Track::Px(32.0)])
        .children([header, sidebar, main, aside, footer])
}

/// Nested scrolling: an outer scrollable column of cards, one of which
/// contains an inner scrollable list. Ids keep both offsets stable.
pub fn scroll_demo<Msg>(theme: &Theme) -> Element<Msg> {
    let card = |i: usize| {
        col()
            .p(SP3)
            .gap(SP1)
            .rounded(R_LG)
            .bg(theme.surface_raised)
            .border(1.0, theme.border_subtle)
            .shadow(ShadowToken::Xs)
            .shrink0()
            .children([
                text(format!("Card {i}"))
                    .weight(Weight::Medium)
                    .size(TextSize::Sm),
                text("Scroll the page; this card scrolls with it.")
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            ])
    };

    let inner_list = col()
        .id("inner-scroll")
        .scroll_y()
        .h(120.0)
        .p(SP2)
        .gap(SP1)
        .rounded(R_SM)
        .bg(theme.surface)
        .border(1.0, theme.border)
        .children((1..=20).map(|i| {
            row().shrink0().px(SP2).h(24.0).items_center().children([
                div().w(6.0).h(6.0).rounded_full().bg(theme.accent),
                text(format!("  inner row {i}")).size(TextSize::Xs),
            ])
        }));

    col()
        .id("outer-scroll")
        .scroll_y()
        .w_full()
        .h_full()
        .p(SP4)
        .gap(SP3)
        .bg(theme.bg)
        .children(
            std::iter::once(
                col().gap(SP1).shrink0().children([
                    text("Nested scrolling")
                        .size(TextSize::Lg)
                        .weight(Weight::Semibold),
                    text("Wheel over the inner list scrolls it; elsewhere scrolls the page.")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                ]),
            )
            .chain((1..=3).map(card))
            .chain(std::iter::once(col().gap(SP1).shrink0().children([
                text("Inner list").size(TextSize::Sm).weight(Weight::Medium),
                inner_list,
            ])))
            .chain((4..=12).map(card)),
        )
}

/// An editorial "study guide cover" poster: the demo that fenestra's
/// design range goes beyond SaaS dashboards. Register a display face under
/// `FamilyRole::Display` and an italic under `FamilyRole::Serif`
/// (`Fonts::register`), build a duotone theme, and render — every color
/// still routes through theme tokens.
pub fn poster<Msg: 'static>(theme: &Theme) -> Element<Msg> {
    use fenestra_core::{FamilyRole, GradientStop, Length, Paint, SP2, SP4, SP6, SP8, stack};

    let field = Paint::LinearGradient {
        angle_deg: 8.0,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: theme.neutrals.step(3),
            },
            GradientStop {
                offset: 0.55,
                color: theme.neutrals.step(2),
            },
            GradientStop {
                offset: 1.0,
                color: theme.neutrals.step(1),
            },
        ],
    };

    // Faint ruled lines: paper grain.
    let rules = (1..18).map(|i| {
        #[expect(clippy::cast_precision_loss, reason = "small loop index")]
        fenestra_core::div::<Msg>()
            .absolute()
            .top(72.0 * i as f32)
            .left(0.0)
            .w(Length::Pct(100.0))
            .h(1.0)
            .themed(|t: &Theme, s| s.bg(t.neutrals.step(4).with_alpha(0.35)))
    });

    // A botanical sprig, drawn once and echoed at two scales.
    let sprig = |w: f32, h: f32| {
        fenestra_core::path::<Msg>(branch_path(), (260.0, 360.0), Some(2.2))
            .w(w)
            .h(h)
            .themed(|t: &Theme, s| s.color(t.neutrals.step(6).with_alpha(0.55)))
    };

    let content = col()
        .absolute()
        .top(0.0)
        .left(0.0)
        .w(Length::Pct(100.0))
        .h(Length::Pct(100.0))
        .pl(150.0)
        .pr(110.0)
        .pt(258.0)
        .items_start()
        .children([
            text("YEAR 8 · INTEGRATED SCIENCE")
                .family(FamilyRole::Mono)
                .size_px(15.0)
                .tracking(0.42)
                .themed(|t: &Theme, s| s.color(t.accents.step(10))),
            row().items_center().gap(SP4).pt(SP6).children([
                fenestra_core::div()
                    .w(88.0)
                    .h(2.0)
                    .themed(|t: &Theme, s| s.bg(t.accents.step(9))),
                text("UNIT EIGHT")
                    .family(FamilyRole::Mono)
                    .size_px(14.0)
                    .tracking(0.42)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            ]),
            text("Evolution")
                .family(FamilyRole::Display)
                .size_px(148.0)
                .leading(1.02)
                .weight(Weight::Semibold)
                .pt(SP8)
                .themed(|t: &Theme, s| s.color(t.text)),
            text("a field guide to deep time")
                .family(FamilyRole::Serif)
                .size_px(56.0)
                .leading(1.1)
                .pt(SP2)
                .themed(|t: &Theme, s| s.color(t.accents.step(9))),
            fenestra_core::div()
                .w(430.0)
                .h(3.0)
                .mt(44.0)
                .themed(|t: &Theme, s| s.bg(t.accents.step(9))),
            text(
                "Everything you need for the end-of-unit test — extinction and \
                 the fossil record, variation, natural selection, and a \
                 changing Earth — explained from the ground up, with worked \
                 examples, diagrams and exam technique.",
            )
            .family(FamilyRole::Display)
            .size_px(23.0)
            .leading(1.72)
            .w(620.0)
            .mt(52.0)
            .themed(|t: &Theme, s| s.color(t.text_muted)),
        ]);

    stack()
        .w(Length::Pct(100.0))
        .h(Length::Pct(100.0))
        .children([fenestra_core::div()
            .w(Length::Pct(100.0))
            .h(Length::Pct(100.0))
            .bg(field)])
        .children(rules)
        .children([
            // The ochre spine.
            fenestra_core::div()
                .absolute()
                .top(0.0)
                .left(40.0)
                .w(18.0)
                .h(Length::Pct(100.0))
                .themed(|t: &Theme, s| s.bg(t.accents.step(9))),
            sprig(330.0, 460.0).absolute().top(820.0).left(640.0),
            sprig(210.0, 290.0).absolute().top(990.0).left(430.0),
            content,
        ])
}

/// A hand-drawn botanical branch in a 260x360 viewbox: one stem, a few
/// arcs, seed dots at the tips.
fn branch_path() -> kurbo::BezPath {
    use kurbo::Shape;
    let mut p = kurbo::BezPath::new();
    // Main stem.
    p.move_to((130.0, 350.0));
    p.curve_to((120.0, 260.0), (140.0, 180.0), (128.0, 80.0));
    // Side branches with a seed dot at each tip.
    type Branch = ((f64, f64), (f64, f64), (f64, f64));
    let branches: [Branch; 6] = [
        ((128.0, 300.0), (80.0, 270.0), (44.0, 246.0)),
        ((126.0, 252.0), (180.0, 226.0), (216.0, 196.0)),
        ((130.0, 206.0), (84.0, 178.0), (58.0, 142.0)),
        ((132.0, 162.0), (186.0, 138.0), (222.0, 104.0)),
        ((129.0, 122.0), (94.0, 96.0), (76.0, 62.0)),
        ((128.0, 80.0), (150.0, 44.0), (170.0, 22.0)),
    ];
    for (start, ctrl, tip) in branches {
        p.move_to(start);
        p.quad_to(ctrl, tip);
        let r = 5.0;
        p.move_to((tip.0 + r, tip.1));
        p.extend(kurbo::Circle::new(tip, r).path_elements(0.1));
    }
    p
}

/// A Figma-style inspector panel: the editor-chrome token tier in one screen —
/// dense 11–13px labels ([`ChromeText`]), 32px control rows, and the floating
/// two-drop + 0.5px hairline-ring elevation ([`ChromeElevation::Popover`]).
/// The visual proof that fenestra can dress as a creative tool, not just a
/// SaaS app. Pair it with the [`canvas`](fenestra_core::canvas) substrate.
pub fn editor_panel<Msg>(_theme: &Theme) -> Element<Msg> {
    let label = |s: &str| {
        text(s.to_owned())
            .size_px(ChromeText::Sm.px())
            .tracking(ChromeText::Sm.tracking())
            .themed(|t: &Theme, st| st.color(t.text_muted))
    };
    let value = |s: &str| {
        text(s.to_owned())
            .size_px(ChromeText::Sm.px())
            .tracking(ChromeText::Sm.tracking())
            .themed(|t: &Theme, st| st.color(t.text))
    };
    let field = |name: &str, val: &str| {
        row()
            .h(32.0)
            .items_center()
            .justify_between()
            .px(SP3)
            .children((label(name), value(val)))
    };

    col()
        .w(240.0)
        .m(SP4)
        .rounded(R_SM)
        .py(SP2)
        // The panel's own elevation IS the editor-chrome vocabulary: two soft
        // drops over a 0.5px hairline ring, flat black (not the themed hue).
        .themed(|t: &Theme, mut s| {
            s.shadows = ChromeElevation::Popover.shadows();
            s.bg(t.surface_raised).border(1.0, t.border_subtle)
        })
        .children((
            row()
                .h(32.0)
                .items_center()
                .px(SP3)
                .children([text("Layout".to_owned())
                    .size_px(ChromeText::Lg.px())
                    .tracking(ChromeText::Lg.tracking())
                    .weight(Weight::Semibold)
                    .themed(|t: &Theme, st| st.color(t.text))]),
            divider(),
            field("Width", "240"),
            field("Height", "auto"),
            field("Opacity", "100%"),
            field("Corner radius", "6"),
        ))
}
