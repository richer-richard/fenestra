//! The OpenType font-feature specimen: figure shape (lining vs old-style),
//! small capitals, fractions, and figure spacing (tabular vs proportional),
//! each shown side by side against the font's default. Figure shape,
//! small caps, and fractions render in the Serif role (a registered serif
//! face carries `onum`/`lnum`/`smcp`/`liga`); figure spacing renders in the
//! Sans role (Inter carries `tnum`/`pnum`). Font-agnostic: roles that lack a
//! face fall back to Inter, where the unsupported features are no-ops.

use fenestra_core::{
    Element, FamilyRole, R_MD, SP1, SP2, SP4, SP6, SP8, TextSize, Theme, Weight, col, div, row,
    text,
};

/// Builds the OpenType font-feature specimen for a theme.
pub fn font_feature_specimen<Msg>(theme: &Theme) -> Element<Msg> {
    let label = move |s: &str| {
        text::<Msg>(s.to_owned())
            .size(TextSize::Xs)
            .weight(Weight::Medium)
            .color(theme.text_muted)
    };
    let title = move |s: &str| {
        text::<Msg>(s.to_owned())
            .size(TextSize::Sm)
            .weight(Weight::Semibold)
            .color(theme.text)
    };

    // One comparison: a titled block with a "default" cell and a
    // "feature on" cell stacked, each captioned.
    let pair =
        move |heading: &str, off_cap: &str, off: Element<Msg>, on_cap: &str, on: Element<Msg>| {
            col().gap(SP2).children([
                title(heading),
                row().gap(SP8).items_baseline().children([
                    col().gap(SP1).children([label(off_cap), off]),
                    col().gap(SP1).children([label(on_cap), on]),
                ]),
            ])
        };

    col().p(SP8).gap(SP6).bg(theme.bg).children([
        text::<Msg>("OpenType font features")
            .size(TextSize::Xl)
            .weight(Weight::Semibold)
            .color(theme.text),
        // Figure shape (Serif): lining vs old-style digits.
        pair(
            "Figures — lining vs old-style (serif)",
            "lining",
            text("0123456789")
                .family(FamilyRole::Serif)
                .size_px(48.0)
                .lining_nums(),
            "old-style",
            text("0123456789")
                .family(FamilyRole::Serif)
                .size_px(48.0)
                .oldstyle_nums(),
        ),
        // Small capitals (Serif).
        pair(
            "Small caps (serif)",
            "default",
            text("Small Caps").family(FamilyRole::Serif).size_px(40.0),
            "small-caps",
            text("Small Caps")
                .family(FamilyRole::Serif)
                .size_px(40.0)
                .small_caps(),
        ),
        // Fractions (Serif).
        pair(
            "Fractions (serif)",
            "default",
            text("1/2 3/4 7/8").family(FamilyRole::Serif).size_px(40.0),
            "fractions",
            text("1/2 3/4 7/8")
                .family(FamilyRole::Serif)
                .size_px(40.0)
                .fractions(),
        ),
        // Figure spacing (Sans/Inter): proportional vs tabular. Mixed-width
        // digits make the alignment delta visible — tabular right edges line
        // up, proportional are ragged.
        col().gap(SP2).children([
            title("Spacing — proportional vs tabular (sans)"),
            row().gap(SP8).children([
                div()
                    .bg(theme.surface)
                    .rounded(R_MD)
                    .p(SP4)
                    .children([col().gap(SP1).children([
                        label("proportional"),
                        text("1 7 1 1").size_px(32.0).proportional_nums(),
                        text("8 8 8 8").size_px(32.0).proportional_nums(),
                    ])]),
                div()
                    .bg(theme.surface)
                    .rounded(R_MD)
                    .p(SP4)
                    .children([col().gap(SP1).children([
                        label("tabular"),
                        text("1 7 1 1").size_px(32.0).tabular(),
                        text("8 8 8 8").size_px(32.0).tabular(),
                    ])]),
            ]),
        ]),
    ])
}
