//! Squircle corner A/B eyeball golden: two identically-sized, identically-
//! rounded boxes side by side, differing only in `corner_smoothing`. The left
//! is the default exact circular arc (smoothing 0.0); the right is a fuller
//! continuous-curvature squircle (smoothing 0.6). Same straight-edge extents,
//! visibly different corners — the whole point of the knob in one PNG.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, col, row};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (420, 220);

/// Two 160×160 raised boxes on a `theme.surface` panel: identical radius
/// (40px), fill, and border — only `corner_smoothing` differs (0.0 vs 0.6).
fn squircle_demo<Msg>(t: &Theme) -> Element<Msg> {
    row()
        .gap(SP6)
        .p(SP6)
        .items_center()
        .justify_center()
        .w_full()
        .h_full()
        .bg(t.surface)
        .children([
            col()
                .w(160.0)
                .h(160.0)
                .rounded(40.0)
                .corner_smoothing(0.0)
                .bg(t.surface_raised)
                .border(1.0, t.border),
            col()
                .w(160.0)
                .h(160.0)
                .rounded(40.0)
                .corner_smoothing(0.6)
                .bg(t.surface_raised)
                .border(1.0, t.border),
        ])
}

#[test]
fn squircle_corners() {
    let theme = Theme::light();
    let image = render_element(squircle_demo::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "squircle_corners", &image);
}
