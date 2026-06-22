//! Menubar golden: a full-width application menu bar of top-level triggers
//! (menus closed at rest). Light + dark — verifies the strip frame, the
//! bottom border, and the trigger labels.

use std::path::PathBuf;

use fenestra_core::{Element, Theme, col};
use fenestra_kit::menubar;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (420, 84);

fn view(theme: &Theme) -> Element<()> {
    let bar: Element<()> = menubar()
        .menu("File", [("New", ()), ("Open", ()), ("Save", ())])
        .menu("Edit", [("Undo", ()), ("Redo", ())])
        .menu("View", [("Zoom In", ()), ("Zoom Out", ())])
        .menu("Help", [("About", ())])
        .into();
    col().bg(theme.bg).children([bar])
}

#[test]
fn menubar_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menubar_light", &image);
}

#[test]
fn menubar_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menubar_dark", &image);
}
