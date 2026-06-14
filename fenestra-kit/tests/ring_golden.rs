//! Ring-border eyeball golden: the "ring, not border" look. Three cards — a 1px
//! stroked `.border()`, a 1px `.ring()` (outer hairline), and a 2px accent
//! `.ring()` (a selection ring) — over a surface. The ring sits just outside
//! the box hugging the corner radius, where the stroke sits on the edge; the
//! ring never eats into the card's content. Light only (geometry).

use std::path::PathBuf;

use fenestra_core::{Element, R_LG, SP4, SP6, Theme, Weight, col, row, text};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

fn card(theme: &Theme, label: &str) -> Element<()> {
    col()
        .w(150.0)
        .h(80.0)
        .p(SP4)
        .rounded(R_LG)
        .bg(theme.surface_raised)
        .children([text(label.to_owned())
            .weight(Weight::Medium)
            .color(theme.text)])
}

#[test]
fn ring_showcase_golden() {
    let t = Theme::light();
    let view: Element<()> = row().p(SP6).gap(SP6).bg(t.surface).children([
        card(&t, "border 1px").border(1.0, t.border),
        card(&t, "ring 1px").ring(1.0, t.border),
        card(&t, "ring 2px accent").ring(2.0, t.accent),
    ]);
    let image = render_element(view, &t, (560, 160));
    assert_png_snapshot(snapshot_dir(), "ring_showcase", &image);
}
