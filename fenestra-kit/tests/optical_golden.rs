//! Optical-adjustment eyeball golden: two play buttons side by side. The LEFT
//! triangle is centered by its bounding box (the naive way) and looks shifted
//! toward the flat edge; the RIGHT triangle is centered on its centroid (via
//! `optical::centroid`) and looks truly centered in the circle — the classic
//! play-button correction. Light only (geometry).

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, div, optical, path, row};
use fenestra_shell::{render_element, testing::assert_png_snapshot};
use kurbo::BezPath;

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const VB: f32 = 48.0;

fn triangle(verts: &[(f32, f32)]) -> BezPath {
    let mut p = BezPath::new();
    p.move_to((f64::from(verts[0].0), f64::from(verts[0].1)));
    p.line_to((f64::from(verts[1].0), f64::from(verts[1].1)));
    p.line_to((f64::from(verts[2].0), f64::from(verts[2].1)));
    p.close_path();
    p
}

/// A play triangle inside an accent circle. `optical_center` shifts the triangle
/// so its centroid (not its bounding box) sits at the circle's center.
fn play_circle(theme: &Theme, optical_center: bool) -> Element<()> {
    // Right-pointing triangle whose bounding box is centered in the viewbox.
    let base = [(14.0_f32, 12.0), (14.0, 36.0), (34.0, 24.0)];
    let verts: Vec<(f32, f32)> = if optical_center {
        let (cx, cy) = optical::centroid(&base);
        let (dx, dy) = (VB / 2.0 - cx, VB / 2.0 - cy);
        base.iter().map(|&(x, y)| (x + dx, y + dy)).collect()
    } else {
        base.to_vec()
    };
    let tri =
        path::<()>(triangle(&verts), (f64::from(VB), f64::from(VB)), None).color(theme.on_accent);
    div::<()>()
        .w(120.0)
        .h(120.0)
        .rounded_full()
        .bg(theme.accent)
        .items_center()
        .justify_center()
        .children([tri])
}

#[test]
fn optical_play_button_golden() {
    let t = Theme::light();
    let view: Element<()> = row().p(SP6).gap(SP6).bg(t.surface).children([
        play_circle(&t, false), // bbox-centered — looks shifted left
        play_circle(&t, true),  // centroid-centered — looks centered
    ]);
    let image = render_element(view, &t, (340, 180));
    assert_png_snapshot(snapshot_dir(), "optical_play", &image);
}
