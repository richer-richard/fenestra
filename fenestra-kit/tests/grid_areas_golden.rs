//! Named-area grid golden: `grid_template_areas` lays out an app shell
//! (header / sidebar / main / footer). Each panel is placed only by `grid_area`
//! name; fenestra resolves the names to taffy lines. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, TextSize, Theme, Track, Weight, col, text};
use fenestra_kit::card;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (640, 400);

fn panel(label: &str, area: &str) -> Element<()> {
    card().grid_area(area).children([text(label.to_string())
        .size(TextSize::Lg)
        .weight(Weight::Semibold)])
}

fn view(theme: &Theme) -> Element<()> {
    col()
        .p(SP4)
        .gap(SP4)
        .bg(theme.bg)
        .w_full()
        .h_full()
        .grid_cols([Track::Px(168.0), Track::Fr(1.0)])
        .grid_rows([Track::Px(56.0), Track::Fr(1.0), Track::Px(48.0)])
        .grid_template_areas(["header header", "sidebar main", "footer footer"])
        .children([
            panel("Header", "header"),
            panel("Sidebar", "sidebar"),
            panel("Main content", "main"),
            panel("Footer", "footer"),
        ])
}

#[test]
fn grid_areas_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "grid_areas_light", &image);
}

#[test]
fn grid_areas_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "grid_areas_dark", &image);
}
