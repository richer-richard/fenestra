//! Responsive grid golden: `responsive_grid` lays out cards as `repeat(auto-fit,
//! minmax(min_col, 1fr))` — at 640px with a 160px minimum, three equal columns
//! that wrap to a second row. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, TextSize, Theme, Weight, col, text};
use fenestra_kit::{card, responsive_grid};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (640, 360);

fn view(theme: &Theme) -> Element<()> {
    let cards = (1..=6).map(|i| {
        card().children([
            text(format!("Card {i}"))
                .size(TextSize::Lg)
                .weight(Weight::Semibold),
            text("Adapts to width")
                .size(TextSize::Sm)
                .themed(|t: &Theme, s| s.color(t.text_muted)),
        ])
    });
    col()
        .p(SP6)
        .bg(theme.bg)
        .children([responsive_grid(160.0, cards)])
}

#[test]
fn responsive_grid_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "responsive_grid_light", &image);
}

#[test]
fn responsive_grid_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "responsive_grid_dark", &image);
}
