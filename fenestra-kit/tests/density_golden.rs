//! Density eyeball golden: the same controls ([`density_showcase`]) at all
//! three densities side by side. The PNG must show the Compact column tighter,
//! Spacious roomier, and Comfortable identical to the kit's default metrics —
//! while every column's label text stays the same size (density scales spacing,
//! not type). Density is theme-independent geometry, so one light golden
//! suffices.

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_kit::density_showcase;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

#[test]
fn density_showcase_golden() {
    let theme = Theme::light();
    let image = render_element(density_showcase::<()>(&theme), &theme, (560, 360));
    assert_png_snapshot(snapshot_dir(), "density_showcase", &image);
}
