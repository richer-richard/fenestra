//! Optical sizing (`opsz`) golden: one variable text serif (Fraunces),
//! shown as two optical masters at one render size, plus auto-tracked sizes.
//!
//! The A/B is deliberate (mirrors the 0.18 OKLCH-gradient A/B golden): at a
//! single large size, `opsz 9` is the *text* master (sturdy strokes, low
//! contrast) and `opsz 144` is the *display* master (fine hairlines, high
//! contrast). The only variable is the axis, so the letterform difference is
//! unmistakable. The lower block uses [`OpticalSizing::Auto`] so the axis
//! tracks the rendered size — the everyday usage.

use fenestra_core::{
    Element, FamilyRole, Fonts, OpticalSizing, SP1, SP4, SP6, TextSize, Theme, Weight, col,
    divider, row, text,
};
use fenestra_shell::render_element_with;
use fenestra_shell::testing::assert_png_snapshot;

/// Fraunces, the bundled variable text serif (`opsz` 9–144, `wght` 100–900).
const FRAUNCES: &[u8] = include_bytes!("../assets/Fraunces[opsz,wght].ttf");

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

/// One specimen: the same glyphs at the same size, pinned to a given `opsz`.
fn specimen(label: &str, opsz: f32) -> Element<()> {
    col().gap(SP1).items_center().children((
        text("Rafg")
            .family(FamilyRole::Serif)
            .size_px(72.0)
            .optical(OpticalSizing::Fixed(opsz)),
        text(label.to_string())
            .size(TextSize::Xs)
            .themed(|t: &Theme, s| s.color(t.text_muted)),
    ))
}

fn scene() -> Element<()> {
    col()
        .p(SP6)
        .gap(SP4)
        .themed(|t: &Theme, s| s.bg(t.surface))
        .children((
            text("Optical sizing — one face, two masters")
                .size(TextSize::Lg)
                .weight(Weight::Semibold),
            // A/B: identical size, only the opsz axis differs.
            row().gap(SP6).items_end().children((
                specimen("opsz 9 · text master", 9.0),
                specimen("opsz 144 · display master", 144.0),
            )),
            divider(),
            text("Auto — opsz follows the rendered size")
                .size(TextSize::Sm)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
            text("Aa")
                .family(FamilyRole::Serif)
                .size_px(48.0)
                .optical_auto(),
            text(
                "Body prose set at sixteen pixels lands on the text master, where \
             the serifs are sturdier and the stroke contrast is lower — drawn \
             to stay legible at reading size.",
            )
            .family(FamilyRole::Serif)
            .size(TextSize::Base)
            .optical_auto()
            .max_w(520.0),
        ))
}

#[test]
fn optical_sizing_golden() {
    let mut fonts = Fonts::embedded();
    assert!(
        fonts.register(FamilyRole::Serif, FRAUNCES.to_vec()),
        "Fraunces registers under the Serif role"
    );
    let theme = Theme::light();
    let image = render_element_with(scene(), &theme, (640, 460), &mut fonts);
    assert_png_snapshot(snapshot_dir(), "optical_sizing", &image);
}
