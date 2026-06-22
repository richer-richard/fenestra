//! Rich menu golden: a floating menu panel with leading icons, trailing
//! shortcut hints, a separator rule, and a disabled (dimmed, inert) item.
//! Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, col};
use fenestra_kit::{icons, menu_item, menu_items, menu_separator};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (290, 210);

fn view(theme: &Theme) -> Element<()> {
    let menu: Element<()> = menu_items([
        menu_item("New File")
            .icon(icons::plus())
            .shortcut("⌘N")
            .on_select(()),
        menu_item("Go Home")
            .icon(icons::home())
            .shortcut("⌘\u{2191}")
            .on_select(()),
        menu_separator(),
        menu_item("Find")
            .icon(icons::search())
            .shortcut("⌘F")
            .on_select(()),
        menu_item("Delete")
            .icon(icons::x())
            .shortcut("⌫")
            .disabled(true),
    ])
    .into();
    col().p(SP6).bg(theme.bg).children([menu])
}

#[test]
fn menu_rich_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_rich_light", &image);
}

#[test]
fn menu_rich_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_rich_dark", &image);
}

/// The same menu with the keyboard cursor on "Find" (index 3): an accent-filled
/// row, exercising the APG keyboard navigation path.
fn keyboard_view(theme: &Theme) -> Element<()> {
    let menu: Element<()> = menu_items([
        menu_item("New File")
            .icon(icons::plus())
            .shortcut("⌘N")
            .on_select(()),
        menu_item("Go Home")
            .icon(icons::home())
            .shortcut("⌘\u{2191}")
            .on_select(()),
        menu_separator(),
        menu_item("Find")
            .icon(icons::search())
            .shortcut("⌘F")
            .on_select(()),
        menu_item("Delete")
            .icon(icons::x())
            .shortcut("⌫")
            .disabled(true),
    ])
    .highlighted(Some(3))
    .on_navigate(|_| ())
    .into();
    col().p(SP6).bg(theme.bg).children([menu])
}

#[test]
fn menu_keyboard_light() {
    let theme = Theme::light();
    let image = render_element(keyboard_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_keyboard_light", &image);
}

#[test]
fn menu_keyboard_dark() {
    let theme = Theme::dark();
    let image = render_element(keyboard_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_keyboard_dark", &image);
}
