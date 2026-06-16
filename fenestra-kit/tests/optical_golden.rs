//! Optical-adjustment eyeball goldens, now driven by the `path()` builders
//! ([`Element::optical_center`] / [`Element::optical_overshoot`]) rather than
//! a caller computing the correction by hand.
//!
//! - `optical_play`: two play buttons. The LEFT triangle is bounding-box
//!   centered (naive) and looks shifted toward the flat edge; the RIGHT one is
//!   `.optical_center()`ed onto its centroid and looks truly centered — the
//!   classic play-button nudge. (Byte-identical to the prior hand-computed
//!   version: the builder applies the same centroid shift in viewbox space.)
//! - `optical_overshoot`: a square, a same-size circle (reads smaller), and a
//!   `.optical_overshoot()` circle (reads the same size as the square).
//!
//! Light only — these are geometry.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, div, path, row, text};
use fenestra_shell::{render_element, testing::assert_png_snapshot};
use kurbo::{BezPath, Circle, Rect, Shape};

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

/// A play triangle inside an accent circle. `.optical_center()` shifts the
/// triangle so its centroid (not its bounding box) sits at the circle's center.
fn play_circle(theme: &Theme, optical_center: bool) -> Element<()> {
    // Right-pointing triangle whose bounding box is centered in the viewbox.
    let base = [(14.0_f32, 12.0), (14.0, 36.0), (34.0, 24.0)];
    let mut tri =
        path::<()>(triangle(&base), (f64::from(VB), f64::from(VB)), None).color(theme.on_accent);
    if optical_center {
        tri = tri.optical_center();
    }
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

/// A filled accent shape centered in an 80px tile.
fn tile(theme: &Theme, shape: BezPath, overshoot: bool) -> Element<()> {
    let mut glyph = path::<()>(shape, (f64::from(VB), f64::from(VB)), None).color(theme.accent);
    if overshoot {
        glyph = glyph.optical_overshoot();
    }
    div::<()>()
        .w(80.0)
        .h(80.0)
        .items_center()
        .justify_center()
        .children([glyph])
}

#[test]
fn optical_overshoot_golden() {
    let t = Theme::light();
    // A square and a circle of the same nominal extent in the viewbox.
    let r = f64::from(VB) * 0.4;
    let c = f64::from(VB) / 2.0;
    let circle = || Circle::new((c, c), r).to_path(0.05);
    let square = Rect::new(c - r, c - r, c + r, c + r).to_path(0.05);

    let view: Element<()> = row().p(SP6).gap(SP6).items_end().bg(t.surface).children([
        col_label("square", tile(&t, square, false)),
        col_label("circle", tile(&t, circle(), false)),
        col_label("circle +overshoot", tile(&t, circle(), true)),
    ]);
    let image = render_element(view, &t, (380, 150));
    assert_png_snapshot(snapshot_dir(), "optical_overshoot", &image);
}

/// A tile above a small caption.
fn col_label(label: &str, tile: Element<()>) -> Element<()> {
    use fenestra_core::{TextSize, col};
    col().items_center().gap(8.0).children([
        tile,
        text(label.to_string())
            .size(TextSize::Xs)
            .themed(|t: &Theme, s| s.color(t.text_muted)),
    ])
}
