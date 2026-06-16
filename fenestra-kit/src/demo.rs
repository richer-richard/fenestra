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
    use fenestra_core::{
        FamilyRole, GRADIENT_STEPS, Length, Paint, SP2, SP4, SP6, SP8, oklch_stops, stack,
    };

    // Paper grain: a near-gray neutral wash with explicit, uneven stops
    // (N3 → N2 at 0.55 → N1), OKLCH-expanded so the subtle ramp stays clean.
    let field = Paint::LinearGradient {
        angle_deg: 8.0,
        stops: oklch_stops(
            &[
                (0.0, theme.neutrals.step(3)),
                (0.55, theme.neutrals.step(2)),
                (1.0, theme.neutrals.step(1)),
            ],
            GRADIENT_STEPS,
        ),
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

/// An AI-chat reading view — the showcase for "the GUI framework AI agents can
/// see," meant to wear the warm-editorial (Claude-like) look. It demonstrates
/// the chat vocabulary: a reading column capped at the default measure
/// (~66ch, resolved against the column's 20px prose); turn asymmetry (the human
/// speaks in a right-aligned accent bubble, the assistant in flat serif prose,
/// no bubble); a blinking streaming caret on the in-progress reply; and a
/// "thinking" shimmer. Register a serif under `FamilyRole::Serif` (the warm
/// look does) for the prose voice; without one it falls back to the sans.
pub fn ai_chat<Msg: 'static>(_theme: &Theme) -> Element<Msg> {
    use fenestra_core::{FamilyRole, Keyframes, Length, MEASURE_CH, SP5, SP6, rich_text, span};

    // The human turn: a right-aligned accent bubble.
    let user = |s: &str| {
        row().w_full().justify_end().child(
            col()
                .max_w(Length::Px(460.0))
                .px(SP4)
                .py(SP3)
                .rounded(R_LG)
                .themed(|t: &Theme, st| st.bg(t.accent_bg))
                .child(
                    text(s.to_owned())
                        .size(TextSize::Base)
                        .themed(|t: &Theme, st| st.color(t.accent_text)),
                ),
        )
    };

    // The assistant turn: flat serif prose, full column, no bubble.
    let assistant = |s: &str| {
        rich_text::<Msg>([span(s).family(FamilyRole::Serif)])
            .w_full()
            .size(TextSize::Lg)
            .leading(1.6)
            .themed(|t: &Theme, st| st.color(t.text))
    };

    // A thin accent block that blinks at the end of the streaming line.
    let caret = div::<Msg>()
        .w(3.0)
        .h(22.0)
        .rounded(1.5)
        .themed(|t: &Theme, s| s.bg(t.accent))
        .keyframes(
            Keyframes::new(1060.0)
                .stop(0.0, |s| s.opacity(1.0))
                .stop(0.5, |s| s.opacity(1.0))
                .stop(0.5001, |s| s.opacity(0.0))
                .stop(1.0, |s| s.opacity(0.0)),
        );

    // A "thinking" shimmer: three dots breathing.
    let shimmer = row().gap(SP2).items_center().children((0..3).map(|_| {
        div::<Msg>()
            .w(7.0)
            .h(7.0)
            .rounded_full()
            .themed(|t: &Theme, s| s.bg(t.text_muted))
            .keyframes(
                Keyframes::new(1200.0)
                    .stop(0.0, |s| s.opacity(0.3))
                    .stop(0.5, |s| s.opacity(1.0))
                    .stop(1.0, |s| s.opacity(0.3)),
            )
    }));

    let streaming = row().w_full().items_center().gap(SP1).children((
        rich_text::<Msg>(
            [span("Because color resolves at construction").family(FamilyRole::Serif)],
        )
        .size(TextSize::Lg)
        .themed(|t: &Theme, st| st.color(t.text)),
        caret,
    ));

    // The column's own text style drives the `ch` measure, so match it to the
    // assistant prose it wraps (20px serif) — not the sans default.
    let column = col()
        .size(TextSize::Lg)
        .family(FamilyRole::Serif)
        .measure(MEASURE_CH)
        .gap(SP5)
        .children([
        assistant("Ask me anything — I render what I build, then look at it, the way an agent reads its own output."),
        user("Explain APCA in one line."),
        assistant("APCA predicts how legible text will be from the lightness contrast between it and its background — which is why fenestra can prove a theme readable before it ever paints a pixel."),
        user("And the streaming feels alive."),
        streaming,
        shimmer,
    ]);

    col()
        .w_full()
        .items_center()
        .px(SP6)
        .py(SP6)
        .themed(|t: &Theme, s| s.bg(t.bg))
        .children([column])
}

/// A frosted-glass command palette floating over a vivid accent-gradient
/// backdrop: the flagship for the [`Surface::Glass`](fenestra_core::Surface::Glass)
/// translucent [`Material`](fenestra_core::Material). The colorful gradient and
/// the in-flow backdrop card it overlaps are clearly modulated *through* the
/// translucent panel (the status chips sit above it), which itself reads as a
/// distinct frosted surface (vibrancy tint + hairline edge + 1px top sheen +
/// deep shadow) with crisp, legible body text. Shared by the glass golden test.
pub fn glass_showcase<Msg>(theme: &Theme) -> Element<Msg> {
    use fenestra_core::{SP6, Surface};

    // Concentric with the glass panel: the row radius is the panel's outer
    // radius minus its SP1 padding, so rows nest without bulging (0.20 rule).
    let row_radius = Surface::Glass.bundle().radius.inner(SP1);

    // A solid status pill with a contrast-checked label — colorful content the
    // eye can track *through* the glass.
    let chip = |label: &str, fill: fenestra_core::Color| {
        row()
            .items_center()
            .px(SP3)
            .h(28.0)
            .rounded_full()
            .bg(fill)
            .child(
                text(label.to_owned())
                    .size(TextSize::Xs)
                    .weight(Weight::Semibold)
                    .color(theme.text_on(fill)),
            )
    };

    // A command row inside the palette: label on the left, a muted shortcut
    // hint on the right, in theme ink that test-proven clears APCA on the glass.
    let command = |label: &str, hint: &str| {
        row()
            .items_center()
            .justify_between()
            .px(SP3)
            .h(34.0)
            .rounded(row_radius)
            .children([
                text(label.to_owned()).size(TextSize::Sm).color(theme.text),
                text(hint.to_owned())
                    .size(TextSize::Xs)
                    .color(theme.text_muted),
            ])
    };

    // The frosted palette: absolutely positioned and horizontally centered over
    // the backdrop (760-wide window, 460-wide panel → 150px inset), out of flow
    // but not via the Overlay system, so the golden stays self-contained.
    let palette = col()
        .absolute()
        .top(176.0)
        .left(150.0)
        .w(460.0)
        .p(SP1)
        .gap(SP1)
        .surface(Surface::Glass)
        .children([
            // A faux search field: muted prompt in a subtly-filled pill.
            row()
                .items_center()
                .px(SP3)
                .h(40.0)
                .rounded(row_radius)
                .bg(theme.neutrals.step(3).with_alpha(0.55))
                .border(1.0, theme.border_subtle)
                .child(
                    text("Search commands…")
                        .size(TextSize::Sm)
                        .color(theme.text_muted),
                ),
            command("New file", "⌘N"),
            command("Open recent", "⌘O"),
            command("Toggle theme", "⌘⇧L"),
            command("Command palette", "⌘K"),
        ]);

    let card = col()
        .w(360.0)
        .p(SP4)
        .gap(SP1)
        .surface(Surface::Card)
        .children([
            text("Backdrop card")
                .size(TextSize::Base)
                .weight(Weight::Semibold)
                .color(theme.text),
            text("In flow behind the glass — its edges show through the pane.")
                .size(TextSize::Sm)
                .color(theme.text_muted),
        ]);

    col()
        .w_full()
        .h_full()
        .p(SP6)
        .gap(SP4)
        .bg(theme.accent_gradient(135.0))
        .children([
            text("Command palette")
                .size(TextSize::Xl3)
                .weight(Weight::Semibold)
                .color(theme.on_accent),
            text("A frosted pane floating over live, colorful content.")
                .size(TextSize::Base)
                .color(theme.on_accent.with_alpha(0.85)),
            row().gap(SP2).children([
                chip("Danger", theme.danger.solid),
                chip("Warning", theme.warning.solid),
                chip("Success", theme.success.solid),
            ]),
            card,
            palette,
        ])
}

/// A density showcase: the same controls at all three [`Density`] levels —
/// Compact / Comfortable / Spacious — side by side, so the one-knob spacing
/// change is visible in a single frame. Each control is a labeled box sized
/// from [`ControlSize::metrics_at`]; the label font is constant across columns
/// (density scales spacing, not type). Shared by the density golden test.
pub fn density_showcase<Msg: 'static>(theme: &Theme) -> Element<Msg> {
    use crate::widgets::{ButtonVariant, Density, button, select, text_input};
    use fenestra_core::SP6;

    // Real kit widgets, restyled by one knob: `.density(..)`. Comfortable is
    // byte-identical to the default; Compact tightens and Spacious loosens the
    // shared height grid, while every label keeps its legible size.
    let column = |theme: &Theme, name: &str, density: Density| -> Element<Msg> {
        col().gap(SP3).items_start().children([
            text(name.to_owned())
                .size(TextSize::Xs)
                .weight(Weight::Semibold)
                .color(theme.text_muted),
            Element::from(
                button("Save")
                    .variant(ButtonVariant::Primary)
                    .density(density),
            ),
            Element::from(text_input("query").width(150.0).density(density)),
            Element::from(
                select(0, ["Recent", "Oldest", "A–Z"])
                    .width(150.0)
                    .density(density),
            ),
        ])
    };

    col().p(SP6).gap(SP4).bg(theme.bg).children([
        text("Density")
            .size(TextSize::Xl)
            .weight(Weight::Semibold)
            .color(theme.text),
        text("The same kit widgets at Compact / Comfortable / Spacious — one .density() knob.")
            .size(TextSize::Sm)
            .color(theme.text_muted),
        row().gap(SP6).items_start().children([
            column(theme, "COMPACT", Density::Compact),
            column(theme, "COMFORTABLE", Density::Comfortable),
            column(theme, "SPACIOUS", Density::Spacious),
        ]),
    ])
}
