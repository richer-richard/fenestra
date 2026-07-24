//! Hi-DPI headless rendering: the same two-pass pipeline at scale 2.0 —
//! text rasterizes at physical resolution and frosted glass keeps its real
//! backdrop blur. This closes the "agents can't see retina-only
//! regressions" blind spot; the golden is the reference for it.

use fenestra_core::{Theme, col, text};
use fenestra_shell::{render_element_scaled, testing::assert_png_snapshot};

fn snapshot_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn view() -> fenestra_core::Element<()> {
    col().p(16.0).gap(8.0).children((
        text("Retina hairlines").size_px(18.0),
        col()
            .p(12.0)
            .rounded(8.0)
            .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
            .child(text("1px border, 2x output")),
    ))
}

#[test]
fn scaled_render_doubles_physical_size() {
    let theme = Theme::light();
    let img = render_element_scaled(view(), &theme, (240, 120), 2.0);
    assert_eq!((img.width(), img.height()), (480, 240));
}

#[test]
fn hidpi_golden() {
    let theme = Theme::light();
    let img = render_element_scaled(view(), &theme, (240, 120), 2.0);
    assert_png_snapshot(snapshot_dir(), "hidpi_2x", &img);
}

/// Hostile scales fall back to 1.0 instead of panicking or allocating
/// absurd textures.
#[test]
fn hostile_scales_are_sanitized() {
    let theme = Theme::light();
    for scale in [f64::NAN, f64::INFINITY, -3.0, 0.0] {
        let img = render_element_scaled(view(), &theme, (100, 50), scale);
        assert_eq!((img.width(), img.height()), (100, 50), "scale {scale}");
    }
}
