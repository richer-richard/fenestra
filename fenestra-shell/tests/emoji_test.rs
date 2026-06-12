//! Color emoji (COLR/sbix) render through system-font fallback — the
//! empirical resolution of issue #11. Chromatic-pixel assertions stay
//! robust across emoji-font updates (goldens would not). macOS-gated
//! like the other system-font proofs.

#![cfg(target_os = "macos")]

use fenestra_core::{Element, Fonts, Theme, col, text};
use fenestra_shell::render_element_with;

fn chromatic_pixels(img: &image::RgbaImage) -> usize {
    img.pixels()
        .filter(|p| {
            let [r, g, b, _] = p.0;
            i32::from(r.max(g).max(b)) - i32::from(r.min(g).min(b)) > 60
        })
        .count()
}

#[test]
fn color_emoji_render_through_system_fonts() {
    let theme = Theme::light();
    let view = |s: &str| -> Element<()> { col().p(12.0).children([text(s).size_px(28.0)]) };

    let mut system = Fonts::with_system();
    let emoji = render_element_with(view("🎉 🚀 👍 🌈"), &theme, (300, 60), &mut system);
    let ascii = render_element_with(view("plain text ok"), &theme, (300, 60), &mut system);

    let emoji_color = chromatic_pixels(&emoji);
    let ascii_color = chromatic_pixels(&ascii);
    assert!(
        emoji_color > 400,
        "emoji should paint strongly chromatic pixels (got {emoji_color})"
    );
    assert!(
        ascii_color < 50,
        "plain text stays achromatic (got {ascii_color})"
    );

    // Known caveat, pinned so a behavior change surfaces: VS16 emoji-
    // presentation sequences (e.g. U+2764 U+FE0F) currently select the
    // monochrome text glyph through the fallback chain.
    let heart = render_element_with(view("\u{2764}\u{fe0f}"), &theme, (300, 60), &mut system);
    assert!(
        chromatic_pixels(&heart) < 50,
        "VS16 sequences fall back to text presentation today; if this \
         starts failing, the fallback improved — update #11's record"
    );
}
