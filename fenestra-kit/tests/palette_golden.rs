//! Command-palette golden: the modal launcher over its dimmed backdrop. Locks
//! the visual (previously golden-uncovered) now that the panel derives its
//! material from `Surface::Menu` rather than a hand-rolled recipe.

use std::path::PathBuf;

use fenestra_core::{Element, Theme};
use fenestra_kit::command_palette;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

#[test]
fn command_palette_golden() {
    let theme = Theme::light();
    // Empty query ⇒ every command shown; open ⇒ the modal is present.
    let view = Element::from(command_palette(
        "",
        true,
        [
            ("Open file…", ()),
            ("Go to line…", ()),
            ("Toggle theme", ()),
            ("Close window", ()),
        ],
    ));
    let image = render_element(view, &theme, (560, 380));
    assert_png_snapshot(snapshot_dir(), "command_palette", &image);
}
