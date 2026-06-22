//! Accordion golden: a three-section disclosure card with the first section
//! open (chevron rotated to point down, body revealed) and the rest collapsed.
//! Light + dark — verifies the rotated caret, the open body panel, and the
//! hairline dividers between sections.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, TextSize, Theme, col, text};
use fenestra_kit::{accordion, accordion_item};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (440, 268);

fn body(copy: &str) -> Element<()> {
    text(copy.to_owned())
        .size(TextSize::Sm)
        .themed(|t: &Theme, s| s.color(t.text_muted))
}

fn view(theme: &Theme) -> Element<()> {
    let acc: Element<()> = accordion([
        accordion_item("Shipping", body("Orders ship within two business days."))
            .open(true)
            .on_toggle(()),
        accordion_item("Returns", body("Thirty-day returns, no questions asked.")).on_toggle(()),
        accordion_item("Warranty", body("Covered for one year against defects.")).on_toggle(()),
    ])
    .into();
    col().p(SP6).bg(theme.bg).children([acc])
}

#[test]
fn accordion_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "accordion_light", &image);
}

#[test]
fn accordion_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "accordion_dark", &image);
}
