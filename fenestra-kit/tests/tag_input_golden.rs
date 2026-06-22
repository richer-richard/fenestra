//! Tag-input goldens: a token field with three removable chips
//! (`design` / `rust` / `gpu`) and a trailing placeholder field, rendered in
//! both themes. Verifies the chip pills, their `×` remove buttons, the bordered
//! rounded container, and the inline placeholder in one PNG.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, col};
use fenestra_kit::tag_input;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (380, 116);

fn view(theme: &Theme) -> Element<()> {
    let field: Element<()> = tag_input(["design", "rust", "gpu"])
        .placeholder("Add a tag…")
        .on_remove(|_| ())
        .on_add(|_| ())
        .id("tags")
        .into();
    col().p(SP6).bg(theme.bg).children([field])
}

#[test]
fn tag_input_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "tag_input_light", &image);
}

#[test]
fn tag_input_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "tag_input_dark", &image);
}
