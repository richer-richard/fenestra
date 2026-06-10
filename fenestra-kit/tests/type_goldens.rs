//! M2 acceptance: the typography specimen matches its golden in both themes.

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_kit::type_specimen;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (560, 620);

#[test]
fn typography_light() {
    let theme = Theme::light();
    let image = render_element(type_specimen::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "typography_light", &image);
}

#[test]
fn typography_dark() {
    let theme = Theme::dark();
    let image = render_element(type_specimen::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "typography_dark", &image);
}
