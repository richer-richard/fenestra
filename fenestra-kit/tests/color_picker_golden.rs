//! OKLCH color picker goldens: a normal picker, a disabled one, and one at a
//! deliberately extreme OKLCH point that gets gamut-mapped (so the warning
//! badge is visible). Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, SP8, Theme, col, oklch};
use fenestra_kit::color_picker;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (460, 1180);

fn view(theme: &Theme) -> Element<()> {
    let normal: Element<()> = color_picker(oklch(0.65, 0.15, 250.0))
        .label("Accent color")
        .pad_size(160.0)
        .on_change(|_| ())
        .on_text_change(|_, _| ())
        .into();
    let disabled: Element<()> = color_picker(oklch(0.55, 0.1, 30.0))
        .label("Disabled color")
        .pad_size(160.0)
        .disabled(true)
        .on_change(|_| ())
        .into();
    // Near-white with strong chroma: past the sRGB gamut edge at this hue,
    // so the swatch shows the "out of gamut" badge.
    let gamut_mapped: Element<()> = color_picker(oklch(0.95, 0.35, 150.0))
        .label("Gamut-mapped color")
        .pad_size(160.0)
        .on_change(|_| ())
        .into();
    col()
        .p(SP6)
        .gap(SP8)
        .bg(theme.bg)
        .children([normal, disabled, gamut_mapped])
}

#[test]
fn color_picker_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "color_picker_light", &image);
}

#[test]
fn color_picker_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "color_picker_dark", &image);
}
