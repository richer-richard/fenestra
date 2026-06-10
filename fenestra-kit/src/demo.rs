//! M3 demo layouts: the holy-grail app shell (grid + absolute positioning)
//! and a nested scrolling layout. Shared by examples and golden tests.

use fenestra_core::{
    Element, R_LG, R_SM, SP1, SP2, SP3, SP4, ShadowToken, TextSize, Theme, Track, Weight, col, div,
    divider, row, text,
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
