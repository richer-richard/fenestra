//! Submenu goldens: (1) a menu whose "Share" item carries a submenu (the
//! trailing chevron affordance, flyout closed), and (2) an open RightStart
//! flyout anchored to a menu panel, verifying the right-of-anchor placement.
//! Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, Overlay, OverlayMode, OverlayPlacement, SP6, Theme, col};
use fenestra_kit::{icons, menu_item, menu_items, menu_separator};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const MENU_SIZE: (u32, u32) = (300, 224);
const FLYOUT_SIZE: (u32, u32) = (540, 200);

/// A menu with a submenu parent ("Share") — chevron visible, flyout closed.
fn menu_view(theme: &Theme) -> Element<()> {
    let menu: Element<()> = menu_items([
        menu_item("Cut").shortcut("⌘X").on_select(()),
        menu_item("Copy").shortcut("⌘C").on_select(()),
        menu_item("Share").icon(icons::search()).submenu([
            menu_item("Email").on_select(()),
            menu_item("Copy Link").on_select(()),
        ]),
        menu_separator(),
        menu_item("Delete").shortcut("⌫").disabled(true),
    ])
    .into();
    col().p(SP6).bg(theme.bg).children([menu])
}

/// An open RightStart flyout anchored to a menu panel (placement verification).
fn flyout_view(theme: &Theme) -> Element<()> {
    let flyout: Element<()> = menu_items([
        menu_item("Email").on_select(()),
        menu_item("Copy Link").on_select(()),
    ])
    .into();
    let flyout = flyout.overlay(Overlay {
        mode: OverlayMode::Open,
        placement: OverlayPlacement::RightStart { gap: 6.0 },
        backdrop: false,
        trap_focus: false,
    });
    let menu: Element<()> = menu_items([
        menu_item("Cut").on_select(()),
        menu_item("Copy").on_select(()),
        menu_item("Paste").on_select(()),
    ])
    .into();
    // Hug the panel (overlays are content-sized in real use) so the flyout to
    // its right isn't pushed off-canvas.
    col()
        .p(SP6)
        .bg(theme.bg)
        .children([menu.self_start().child(flyout)])
}

#[test]
fn menu_submenu_light() {
    let theme = Theme::light();
    let image = render_element(menu_view(&theme), &theme, MENU_SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_submenu_light", &image);
}

#[test]
fn menu_submenu_dark() {
    let theme = Theme::dark();
    let image = render_element(menu_view(&theme), &theme, MENU_SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_submenu_dark", &image);
}

#[test]
fn menu_flyout_light() {
    let theme = Theme::light();
    let image = render_element(flyout_view(&theme), &theme, FLYOUT_SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_flyout_light", &image);
}

#[test]
fn menu_flyout_dark() {
    let theme = Theme::dark();
    let image = render_element(flyout_view(&theme), &theme, FLYOUT_SIZE);
    assert_png_snapshot(snapshot_dir(), "menu_flyout_dark", &image);
}
