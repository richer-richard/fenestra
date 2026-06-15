//! Per-side border eyeball golden: a bottom rule under a header, a left accent
//! rail on a pull-quote, and a box with all four hairline edges — the
//! ruled-layout vocabulary that previously needed manual 1px divider children.
//! Light only; deterministic.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, SP6, TextSize, Theme, Weight, col, text};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const W: u32 = 440;
const H: u32 = 320;

fn view(theme: &Theme) -> Element<()> {
    col().p(SP6).gap(SP6).bg(theme.bg).children([
        // A header with a single bottom rule (no divider child).
        col()
            .pb(SP4)
            .border_bottom(1.0, theme.border)
            .children([text("Section header")
                .size(TextSize::Lg)
                .weight(Weight::Semibold)
                .color(theme.text)]),
        // A pull-quote with a left accent rail.
        col().pl(SP4).border_left(3.0, theme.accent).children([text(
            "Borders resolve per edge now — a rule, a rail, a frame.",
        )
        .color(theme.text_muted)]),
        // A box framed by four independent hairline edges.
        col()
            .p(SP4)
            .border_top(1.0, theme.border)
            .border_right(1.0, theme.border)
            .border_bottom(1.0, theme.border)
            .border_left(1.0, theme.border)
            .children([text("Four hairline edges.").color(theme.text)]),
    ])
}

#[test]
fn side_borders_golden() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, (W, H));
    assert_png_snapshot(snapshot_dir(), "side_borders", &image);
}
