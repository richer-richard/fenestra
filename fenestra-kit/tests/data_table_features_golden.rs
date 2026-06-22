//! Data-table feature goldens: one table sorted by its last column (the
//! accent-tinted header with a ▼ caret) and put into multi-select with two of
//! four rows ticked — so the leading checkbox column shows two checks, the
//! header select-all sits in the tri-state mixed (dash) state, and the ticked
//! rows take the accent highlight. Light + dark in one shot each.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, col};
use fenestra_kit::data_table;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (560, 260);

fn view(theme: &Theme) -> Element<()> {
    let table: Element<()> = data_table(
        ["Name", "Role", "Commits"],
        vec![
            vec!["Ripley".into(), "Warrant Officer".into(), "128".into()],
            vec!["Dallas".into(), "Captain".into(), "97".into()],
            vec!["Lambert".into(), "Navigator".into(), "64".into()],
            vec!["Kane".into(), "Executive Officer".into(), "42".into()],
        ],
    )
    // Sorted by Commits, descending: the rows are already in that order, so
    // the table only needs to draw the ▼ indicator on the active header.
    .sort(2, false)
    // Two of four rows ticked → select-all is mixed (a dash, not a check).
    .selection([true, false, true, false])
    .on_sort(|_| ())
    .on_select_row(|_| ())
    .on_select_all(())
    .into();
    col().p(SP6).bg(theme.bg).children([table])
}

#[test]
fn data_table_features_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "data_table_features_light", &image);
}

#[test]
fn data_table_features_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "data_table_features_dark", &image);
}
