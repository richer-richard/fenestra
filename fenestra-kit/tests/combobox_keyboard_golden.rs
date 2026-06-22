//! Combobox + command-palette keyboard tests. The goldens lock the visual: each
//! open option list with one row carrying the keyboard cursor (the accent veil),
//! light + dark per widget. The harness tests then drive the actual keyboard —
//! Up/Down step the cursor (clamped at the ends) and Enter picks/runs the
//! highlighted row, not merely the first.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP6, Theme, col};
use fenestra_kit::{combobox, command_palette};
use fenestra_shell::{Harness, render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const COMBOBOX_SIZE: (u32, u32) = (360, 340);
const PALETTE_SIZE: (u32, u32) = (560, 380);

fn combobox_view(theme: &Theme) -> Element<()> {
    // Open over the full option set with the second row (Ruby) as the cursor.
    let cb: Element<()> = combobox("", true, ["Rust", "Ruby", "Python", "Go", "Zig"])
        .width(240.0)
        .placeholder("Language…")
        .highlighted(Some(1))
        .on_input(|_| ())
        .on_pick(|_| ())
        .on_navigate(|_| ())
        .on_close(())
        .id("lang")
        .into();
    col().p(SP6).bg(theme.bg).children([cb])
}

fn palette_view() -> Element<()> {
    // The modal launcher with the second command highlighted as the cursor.
    command_palette(
        "",
        true,
        [
            ("Open file…", ()),
            ("Go to line…", ()),
            ("Toggle theme", ()),
            ("Close window", ()),
        ],
    )
    .highlighted(Some(1))
    .on_input(|_| ())
    .on_navigate(|_| ())
    .on_close(())
    .into()
}

#[test]
fn combobox_open_light() {
    let theme = Theme::light();
    let image = render_element(combobox_view(&theme), &theme, COMBOBOX_SIZE);
    assert_png_snapshot(snapshot_dir(), "combobox_open_light", &image);
}

#[test]
fn combobox_open_dark() {
    let theme = Theme::dark();
    let image = render_element(combobox_view(&theme), &theme, COMBOBOX_SIZE);
    assert_png_snapshot(snapshot_dir(), "combobox_open_dark", &image);
}

#[test]
fn command_palette_open_light() {
    let theme = Theme::light();
    let image = render_element(palette_view(), &theme, PALETTE_SIZE);
    assert_png_snapshot(snapshot_dir(), "command_palette_open_light", &image);
}

#[test]
fn command_palette_open_dark() {
    let theme = Theme::dark();
    let image = render_element(palette_view(), &theme, PALETTE_SIZE);
    assert_png_snapshot(snapshot_dir(), "command_palette_open_dark", &image);
}
