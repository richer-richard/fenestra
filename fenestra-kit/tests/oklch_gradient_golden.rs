//! OKLCH gradient builder A/B eyeball golden: a naive two-stop sRGB cross-hue
//! gradient stacked directly above the OKLCH-expanded one (same anchors), so
//! the gray dead-zone the OKLCH path eliminates is unmistakable in one PNG.
//! Light + dark — the win is the hue arc, not the mode.

use std::path::PathBuf;

use fenestra_core::{
    Element, GradientStop, Paint, R_MD, SP4, SP6, Theme, col, div, linear_gradient,
};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

// Exactly fits the panel: 24px padding + 480 panel + 24, and
// 24 + 80 + 16 gap + 80 + 24 tall — so the col fills the window.
const SIZE: (u32, u32) = (528, 224);

/// Two stacked 480×80 panels filling a `theme.bg` column: the top a naive
/// two-stop sRGB gradient (vello interpolates it straight through gray), the
/// bottom the OKLCH-expanded builder over the *same* accent → warning anchors.
fn panel<Msg>(theme: &Theme) -> Element<Msg> {
    let accent = theme.accent;
    let warning = theme.warning.solid;
    let naive = Paint::LinearGradient {
        angle_deg: 90.0,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: accent,
            },
            GradientStop {
                offset: 1.0,
                color: warning,
            },
        ],
    };
    let perceptual = linear_gradient(90.0, [accent, warning]);
    col().p(SP6).gap(SP4).bg(theme.bg).children([
        div().w(480.0).h(80.0).rounded(R_MD).bg(naive),
        div().w(480.0).h(80.0).rounded(R_MD).bg(perceptual),
    ])
}

#[test]
fn oklch_gradient_light() {
    let theme = Theme::light();
    let image = render_element(panel::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "oklch_gradient_light", &image);
}

#[test]
fn oklch_gradient_dark() {
    let theme = Theme::dark();
    let image = render_element(panel::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "oklch_gradient_dark", &image);
}
