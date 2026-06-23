//! Right-to-left golden: under `Theme::rtl()` the layout mirrors — a `start`
//! heading right-aligns, and a label/value row reverses so the label sits on the
//! right and the value on the left. Latin content keeps it font-independent;
//! the mirroring is what RTL delivers. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP3, SP5, TextSize, Theme, Weight, col, row, spacer, text};
use fenestra_kit::card;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (380, 220);

fn view(theme: &Theme) -> Element<()> {
    col().p(SP5).gap(SP5).bg(theme.bg).children([
        text("Start-aligned heading")
            .size(TextSize::Lg)
            .weight(Weight::Semibold),
        card().children([row().items_center().w_full().px(SP3).children([
            text("Label").weight(Weight::Medium),
            spacer(),
            text("Value").themed(|t: &Theme, s| s.color(t.text_muted)),
        ])]),
    ])
}

#[test]
fn rtl_light() {
    let theme = Theme::light().rtl();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "rtl_light", &image);
}

#[test]
fn rtl_dark() {
    let theme = Theme::dark().rtl();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "rtl_dark", &image);
}
