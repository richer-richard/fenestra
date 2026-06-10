//! The typography specimen: the size scale, weights, semantic text colors,
//! mono, wrapping, truncation, and baseline alignment of mixed sizes.

use fenestra_core::{
    Element, R_SM, SP1, SP2, SP4, SP6, TextSize, Theme, Weight, col, div, row, text,
};

/// Builds the typography specimen for a theme.
pub fn type_specimen<Msg>(theme: &Theme) -> Element<Msg> {
    let scale = [
        (TextSize::Xs, "Xs 12 — The quick brown fox"),
        (TextSize::Sm, "Sm 14 — The quick brown fox"),
        (TextSize::Base, "Base 16 — The quick brown fox"),
        (TextSize::Lg, "Lg 20 — The quick brown fox"),
        (TextSize::Xl, "Xl 25 — The quick brown fox"),
        (TextSize::Xl2, "Xl2 31 — Quick brown fox"),
        (TextSize::Xl3, "Xl3 39 — Quick brown"),
    ];

    col().p(SP6).gap(SP4).bg(theme.bg).children([
        // The size scale with token line heights and letter spacing.
        col()
            .gap(SP1)
            .children(scale.into_iter().map(|(size, s)| text(s).size(size))),
        // Weights at Base.
        row().gap(SP4).items_baseline().children([
            text("Regular 400").weight(Weight::Regular),
            text("Medium 500").weight(Weight::Medium),
            text("Semibold 600").weight(Weight::Semibold),
        ]),
        // Semantic text colors.
        row().gap(SP4).items_baseline().children([
            text("text"),
            text("muted").color(theme.text_muted),
            text("subtle").color(theme.text_subtle),
            text("disabled").color(theme.text_disabled),
            text("accent").color(theme.accent_text),
            text("danger").color(theme.danger.text),
        ]),
        // Mono role (falls back to Inter in headless embedded-only mode).
        text("mono 0123456789 () {} =>").mono().size(TextSize::Sm),
        // Wrapping at a fixed width, and single-line ellipsis truncation.
        row().gap(SP4).children([
            div()
                .w(240.0)
                .bg(theme.surface)
                .p(SP2)
                .rounded(R_SM)
                .children([text(
                    "Wrapping: web aesthetics without the web platform. Layered \
                 soft shadows, OKLCH ramps, and real typographic hierarchy.",
                )
                .size(TextSize::Sm)]),
            div()
                .w(200.0)
                .bg(theme.surface)
                .p(SP2)
                .rounded(R_SM)
                .children([
                    text("Truncation: this sentence is far too long to fit on one line")
                        .size(TextSize::Sm)
                        .truncate(),
                ]),
        ]),
        // Baseline-correct alignment of mixed sizes (plus a synthesized
        // box baseline at its bottom edge).
        row().gap(SP2).items_baseline().children([
            text("Xl3").size(TextSize::Xl3).weight(Weight::Semibold),
            text("baseline").size(TextSize::Base),
            text("Xs").size(TextSize::Xs).color(theme.text_muted),
            div().w(16.0).h(16.0).rounded(4.0).bg(theme.accent),
        ]),
    ])
}
