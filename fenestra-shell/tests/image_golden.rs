//! The image element: RGBA8 pixels drawn into the element rect, clipped to
//! the corner radius, with intrinsic sizing by default.

use std::path::PathBuf;

use fenestra_core::{Element, SP3, Theme, col, image_rgba8, row};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// A deterministic RG gradient with solid blue and full alpha.
fn gradient(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            #[expect(clippy::cast_possible_truncation, reason = "ratios are in 0..=255")]
            {
                px.push((x * 255 / w.max(1)) as u8);
                px.push((y * 255 / h.max(1)) as u8);
            }
            px.push(128);
            px.push(255);
        }
    }
    px
}

/// Intrinsic size, stretched, and a round avatar crop, all from one source.
#[test]
fn image_basic_golden() {
    let theme = Theme::light();
    let view: Element<()> = col().p(SP3).gap(SP3).items_start().children([
        image_rgba8(48, 32, gradient(48, 32)),
        image_rgba8(48, 32, gradient(48, 32)).w(120.0).h(24.0),
        row().gap(SP3).children([
            image_rgba8(40, 40, gradient(40, 40)).rounded_full(),
            image_rgba8(40, 40, gradient(40, 40)).rounded(8.0),
        ]),
    ]);
    let image = render_element(view, &theme, (180, 160));
    assert_png_snapshot(snapshot_dir(), "image_basic", &image);
}

/// Pixel data shorter than width*height*4 drops incomplete rows instead of
/// panicking; the element shrinks to the rows actually provided.
#[test]
fn image_short_data_is_safe() {
    let theme = Theme::light();
    // 10x10 requested, but only 4 complete rows of data supplied.
    let view: Element<()> = col().children([image_rgba8(10, 10, vec![200; 10 * 4 * 4])]);
    let image = render_element(view, &theme, (40, 40));
    assert_eq!(image.dimensions(), (40, 40));
}
