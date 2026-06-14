//! Effect-nodes eyeball golden: a mesh-gradient field (OKLab-blended from four
//! accent-ramp points — the "liquid light" look) with a fine film-grain overlay
//! and a label on top. Proves both generated textures render: the field is
//! smooth and vivid (no gray dead-zone), the grain adds subtle tactile noise,
//! and label text stays legible. Deterministic (seeded grain), so it
//! golden-locks. Light only.

use std::path::PathBuf;

use fenestra_core::{
    Element, SP6, TextSize, Theme, Weight, col,
    effects::{MeshPoint, grain, mesh},
    image_rgba8, stack, text,
};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const W: u32 = 480;
const H: u32 = 240;

fn view(theme: &Theme) -> Element<()> {
    // Four accent-ramp points — every color is a theme token.
    let points = [
        MeshPoint {
            x: 0.08,
            y: 0.15,
            color: theme.accent,
        },
        MeshPoint {
            x: 0.92,
            y: 0.10,
            color: theme.accents.step(6),
        },
        MeshPoint {
            x: 0.72,
            y: 0.92,
            color: theme.accent_hover,
        },
        MeshPoint {
            x: 0.18,
            y: 0.85,
            color: theme.accents.step(8),
        },
    ];
    stack().children([
        image_rgba8::<()>(W, H, mesh(W, H, &points)),
        image_rgba8::<()>(W, H, grain(W, H, 7, 0.06)),
        col().p(SP6).children([text("Mesh gradient + grain")
            .size(TextSize::Lg)
            .weight(Weight::Semibold)
            .color(theme.on_accent)]),
    ])
}

#[test]
fn effects_showcase_golden() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, (W, H));
    assert_png_snapshot(snapshot_dir(), "effects_showcase", &image);
}
