//! Glass material eyeball golden: a frosted command palette
//! ([`glass_showcase`]) floating over a vivid accent-gradient backdrop, light +
//! dark. The PNG must show three things at once — the colorful backdrop and the
//! in-flow backdrop card it overlaps are blurred and vibrancy-tinted *through*
//! the pane (the frosting reads; the status chips sit above it), the panel reads
//! as a distinct frosted surface (hairline edge + 1px top sheen + `Lg` shadow),
//! and the panel's body text stays crisp and legible. The backdrop blur is a
//! real two-pass CPU blur in headless rendering; see ARCHITECTURE.md ("Real
//! frosted-glass backdrop blur").

use std::path::PathBuf;

use fenestra_core::Theme;
use fenestra_kit::glass_showcase;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (760, 560);

#[test]
fn glass_showcase_light() {
    let theme = Theme::light();
    let image = render_element(glass_showcase::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "glass_showcase_light", &image);
}

#[test]
fn glass_showcase_dark() {
    let theme = Theme::dark();
    let image = render_element(glass_showcase::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "glass_showcase_dark", &image);
}
