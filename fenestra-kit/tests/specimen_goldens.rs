//! M1 acceptance: the painting specimen matches its golden in both themes.

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_kit::specimen;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (760, 560);

#[test]
fn specimen_light() {
    let theme = Theme::light();
    let image = render_element(specimen::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "specimen_light", &image);
}

#[test]
fn specimen_dark() {
    let theme = Theme::dark();
    let image = render_element(specimen::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "specimen_dark", &image);
}
