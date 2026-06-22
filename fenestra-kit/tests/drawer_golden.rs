//! Drawer/sheet goldens: a left side drawer and a bottom sheet, each open over
//! dimmed page content. Light + dark — verifies the edge-flush panel (flat on
//! the anchored edge, rounded on the inner corners), the backdrop scrim, the
//! full-span sizing, and the header with its close button.

use std::path::PathBuf;

use fenestra_core::{DrawerSide, Element, SP2, SP4, SP6, TextSize, Theme, Weight, col, text};
use fenestra_kit::{button, drawer};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (520, 360);

fn body_line(copy: &str) -> Element<()> {
    text(copy.to_owned())
        .size(TextSize::Sm)
        .themed(|t: &Theme, s| s.color(t.text_muted))
}

/// Page content with the drawer/sheet `panel` layered over it as an overlay.
fn page(theme: &Theme, panel: Element<()>) -> Element<()> {
    col()
        .w_full()
        .h_full()
        .p(SP6)
        .gap(SP4)
        .bg(theme.bg)
        .children([
            text("Dashboard")
                .size(TextSize::Lg)
                .weight(Weight::Semibold)
                .themed(|t: &Theme, s| s.color(t.text)),
            body_line("Content sits behind the dimmed backdrop."),
        ])
        .child(panel)
}

fn left_view(theme: &Theme) -> Element<()> {
    let d: Element<()> = drawer(DrawerSide::Left)
        .title("Filters")
        .size(280.0)
        .child(col().gap(SP2).children([
            body_line("Status: Active"),
            body_line("Owner: Anyone"),
            body_line("Updated: This week"),
        ]))
        .child(button("Apply").on_click(()))
        .on_close(())
        .into();
    page(theme, d)
}

fn bottom_view(theme: &Theme) -> Element<()> {
    let d: Element<()> = drawer(DrawerSide::Bottom)
        .title("Share")
        .size(170.0)
        .child(body_line("Anyone with the link can view this document."))
        .child(button("Copy link").on_click(()))
        .on_close(())
        .into();
    page(theme, d)
}

#[test]
fn drawer_left_light() {
    let theme = Theme::light();
    let image = render_element(left_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "drawer_left_light", &image);
}

#[test]
fn drawer_left_dark() {
    let theme = Theme::dark();
    let image = render_element(left_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "drawer_left_dark", &image);
}

#[test]
fn drawer_bottom_light() {
    let theme = Theme::light();
    let image = render_element(bottom_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "drawer_bottom_light", &image);
}

#[test]
fn drawer_bottom_dark() {
    let theme = Theme::dark();
    let image = render_element(bottom_view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "drawer_bottom_dark", &image);
}
