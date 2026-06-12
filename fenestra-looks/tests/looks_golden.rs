//! One sample screen, three voices — golden-locked so each look's
//! identity is pinned, not aspirational.

use fenestra_core::{
    Element, FamilyRole, Mode, R_MD, SP2, SP3, SP4, Semantics, ShadowToken, TextSize, Theme,
    Weight, col, div, rich_text, row, span, text,
};
use fenestra_looks::{Look, all, editorial, product, terminal};
use fenestra_shell::render_element_with;
use fenestra_shell::testing::assert_png_snapshot;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

/// A small editorial card exercising display type, body text, accent,
/// and surfaces — enough to see a look's whole personality.
fn sample(_theme: &Theme) -> Element<()> {
    col().p(SP4).gap(SP3).children((
        text("Field Notes")
            .family(FamilyRole::Display)
            .size_px(30.0)
            .weight(Weight::Semibold),
        rich_text([
            span("Issue 12 — "),
            span("the verification number")
                .family(FamilyRole::Serif)
                .italic(),
        ])
        .size(TextSize::Sm),
        col()
            .p(SP3)
            .gap(SP2)
            .rounded(R_MD)
            .shadow(ShadowToken::Md)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
            .children((
                text("Every claim in this issue was rendered, queried, and diffed before print.")
                    .size(TextSize::Sm),
                row().gap(SP2).items_center().children((
                    div()
                        .px(SP3)
                        .py(6.0)
                        .rounded(R_MD)
                        .themed(|t: &Theme, s| s.bg(t.accent))
                        .semantics(Semantics::Button)
                        .label("Read")
                        .children([text("Read")
                            .size(TextSize::Sm)
                            .themed(|t: &Theme, s| s.color(t.bg))]),
                    text("12 min")
                        .size(TextSize::Xs)
                        .themed(|t: &Theme, s| s.color(t.text_muted)),
                )),
            )),
    ))
}

fn shoot(look: &Look) -> image::RgbaImage {
    let mut fonts = look.fonts();
    render_element_with(sample(&look.theme), &look.theme, (380, 250), &mut fonts)
}

#[test]
fn product_look_golden() {
    assert_png_snapshot(
        snapshot_dir(),
        "look_product",
        &shoot(&product(Mode::Light)),
    );
}

#[test]
fn editorial_look_golden() {
    assert_png_snapshot(
        snapshot_dir(),
        "look_editorial",
        &shoot(&editorial(Mode::Dark)),
    );
}

#[test]
fn terminal_look_golden() {
    assert_png_snapshot(
        snapshot_dir(),
        "look_terminal",
        &shoot(&terminal(Mode::Dark)),
    );
}

#[test]
fn all_returns_every_look() {
    let names: Vec<&str> = all(Mode::Light).iter().map(|l| l.name).collect();
    assert_eq!(names, vec!["product", "editorial", "terminal"]);
}
