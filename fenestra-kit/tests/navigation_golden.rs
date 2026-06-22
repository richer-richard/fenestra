//! Breadcrumb goldens: a plain trail with a home-icon root and a bold current
//! page, stacked above a long trail collapsed to an ellipsis via `max_items`.
//! Light + dark — verifies the chevron separators, the link/current contrast,
//! and the overflow collapse in one PNG.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, SP6, Theme, col};
use fenestra_kit::{breadcrumbs, crumb, icons};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (620, 140);

fn view(theme: &Theme) -> Element<()> {
    let simple: Element<()> = breadcrumbs([
        crumb("Home").icon(icons::home()).on_select(()),
        crumb("Library").on_select(()),
        crumb("Charts"),
    ])
    .into();
    let collapsed: Element<()> = breadcrumbs([
        crumb("Home").on_select(()),
        crumb("Reports").on_select(()),
        crumb("2026").on_select(()),
        crumb("Q2").on_select(()),
        crumb("June").on_select(()),
        crumb("Summary"),
    ])
    .max_items(4)
    .into();
    col()
        .p(SP6)
        .gap(SP4)
        .bg(theme.bg)
        .children([simple, collapsed])
}

#[test]
fn breadcrumbs_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "breadcrumbs_light", &image);
}

#[test]
fn breadcrumbs_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "breadcrumbs_dark", &image);
}
