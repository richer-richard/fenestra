//! The vendored Lucide icon subset: every icon parses into a non-empty
//! path and the full set is visually locked by one grid golden.

use std::path::PathBuf;

use fenestra_core::{Element, SP3, Theme, col, row};
use fenestra_kit::icons::lucide;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

#[test]
fn lucide_grid_golden() {
    let icons: Vec<Element<()>> = vec![
        lucide::calendar_days(),
        lucide::chevron_up(),
        lucide::filter(),
        lucide::folder(),
        lucide::heart(),
        lucide::link(),
        lucide::lock(),
        lucide::log_out(),
        lucide::refresh_cw(),
        lucide::save(),
        lucide::share_2(),
        lucide::star(),
        lucide::arrow_left(),
        lucide::arrow_right(),
        lucide::bell(),
        lucide::calendar(),
        lucide::clock(),
        lucide::copy(),
        lucide::download(),
        lucide::external_link(),
        lucide::eye(),
        lucide::home(),
        lucide::info(),
        lucide::mail(),
        lucide::menu(),
        lucide::minus(),
        lucide::moon(),
        lucide::pencil(),
        lucide::plus(),
        lucide::search(),
        lucide::settings(),
        lucide::sun(),
        lucide::alert_triangle(),
        lucide::trash(),
        lucide::upload(),
        lucide::user(),
    ];
    let mut grid = col().p(SP3).gap(SP3);
    let mut iter = icons.into_iter().peekable();
    while iter.peek().is_some() {
        grid = grid.child(
            row()
                .gap(SP3)
                .children(iter.by_ref().take(6).collect::<Vec<_>>()),
        );
    }
    let theme = Theme::light();
    let image = render_element(grid, &theme, (240, 170));
    assert_png_snapshot(snapshot_dir(), "lucide_grid", &image);
}
