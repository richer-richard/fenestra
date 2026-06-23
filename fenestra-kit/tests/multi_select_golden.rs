//! Multi-select golden: a wrapping chip set with several options selected
//! (accent-filled with a check) and the rest outlined. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP3, SP5, TextSize, Theme, Weight, col, text};
use fenestra_kit::multi_select;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (440, 200);

fn view(theme: &Theme) -> Element<()> {
    col().p(SP5).gap(SP3).bg(theme.bg).children([
        text("Languages")
            .size(TextSize::Sm)
            .weight(Weight::Semibold)
            .themed(|t: &Theme, s| s.color(t.text_muted)),
        Element::from(multi_select(
            [0, 2, 3],
            ["Rust", "Go", "Zig", "Swift", "Kotlin", "C++", "Elixir"],
        )),
    ])
}

#[test]
fn multi_select_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "multi_select_light", &image);
}

#[test]
fn multi_select_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "multi_select_dark", &image);
}
