//! Toolbar goldens: a horizontal action bar (icon buttons grouped by separator
//! rules, plus a trailing labelled button) above a vertical icon bar. Light +
//! dark — verifies the surface frame, the orientation-aware separator rules, and
//! that grouped controls keep their own chrome.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, SP6, Theme, col};
use fenestra_kit::{button, icon_button, icons, toolbar};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (360, 236);

fn view(theme: &Theme) -> Element<()> {
    let horizontal: Element<()> = toolbar()
        .label("Formatting")
        .item(icon_button(icons::home()).label("Home").on_click(()))
        .item(icon_button(icons::search()).label("Search").on_click(()))
        .separator()
        .item(icon_button(icons::check()).label("Confirm").on_click(()))
        .item(icon_button(icons::x()).label("Cancel").on_click(()))
        .separator()
        .item(button("Done").on_click(()))
        .into();
    let vertical: Element<()> = toolbar()
        .vertical()
        .label("Tools")
        .item(
            icon_button(icons::chevron_left())
                .label("Back")
                .on_click(()),
        )
        .item(
            icon_button(icons::chevron_right())
                .label("Forward")
                .on_click(()),
        )
        .separator()
        .item(icon_button(icons::search()).label("Find").on_click(()))
        .into();
    col()
        .p(SP6)
        .gap(SP4)
        .bg(theme.bg)
        .children([horizontal, vertical])
}

#[test]
fn toolbar_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "toolbar_light", &image);
}

#[test]
fn toolbar_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "toolbar_dark", &image);
}
