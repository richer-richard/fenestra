//! Gradient-shape goldens: the linear, radial, and conic builders rendered side
//! by side over the same accent → warning → success OKLCH ramp, so the angle
//! sweep (linear), the centered falloff (radial), and the hue wheel (conic) are
//! each visually verifiable in a single PNG. Light + dark.

use std::path::PathBuf;

use fenestra_core::{
    Element, R_MD, SP4, SP6, Theme, conic_gradient, div, linear_gradient, radial_gradient, row,
};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

// 24 pad + three 160 tiles + two 16 gaps + 24 pad = 560 wide; 24 + 160 + 24 tall.
const SIZE: (u32, u32) = (560, 208);

/// Three 160×160 tiles filling a `theme.bg` row: a 120° linear sweep, a centered
/// radial falloff, and a centered conic wheel — all over the identical
/// accent → warning → success anchors so the shapes, not the colors, are the
/// thing under test.
fn tiles<Msg>(theme: &Theme) -> Element<Msg> {
    let ramp = [theme.accent, theme.warning.solid, theme.success.solid];
    let linear = linear_gradient(120.0, ramp);
    let radial = radial_gradient((0.5, 0.5), 0.5, ramp);
    let conic = conic_gradient((0.5, 0.5), ramp);
    row().p(SP6).gap(SP4).bg(theme.bg).children([
        div().w(160.0).h(160.0).rounded(R_MD).bg(linear),
        div().w(160.0).h(160.0).rounded(R_MD).bg(radial),
        div().w(160.0).h(160.0).rounded(R_MD).bg(conic),
    ])
}

#[test]
fn gradient_shapes_light() {
    let theme = Theme::light();
    let image = render_element(tiles::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "gradient_shapes_light", &image);
}

#[test]
fn gradient_shapes_dark() {
    let theme = Theme::dark();
    let image = render_element(tiles::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "gradient_shapes_dark", &image);
}
