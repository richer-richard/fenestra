//! Script fallback through system fonts: CJK text shapes through a real
//! family under `Fonts::with_system`, instead of Inter's missing-glyph
//! boxes. Pixel-proven, macOS-gated (the CI reference platform ships
//! PingFang; other runners' font inventories vary).

#![cfg(target_os = "macos")]

use fenestra_core::{Element, Fonts, Theme, col, text};
use fenestra_shell::render_element_with;

fn cjk_view() -> Element<()> {
    col()
        .p(12.0)
        .children([text("evolution 进化论 シンカロン 진화").size_px(22.0)])
}

#[test]
fn system_fonts_provide_cjk_fallback() {
    let theme = Theme::light();
    let mut embedded = Fonts::embedded();
    let a = render_element_with(cjk_view(), &theme, (460, 60), &mut embedded);
    let mut system = Fonts::with_system();
    let b = render_element_with(cjk_view(), &theme, (460, 60), &mut system);

    let differing = a
        .pixels()
        .zip(b.pixels())
        .filter(|(pa, pb)| pa != pb)
        .count();
    assert!(
        differing > 500,
        "system fallback should change the CJK glyphs ({differing} pixels differ)"
    );

    // And the system render must actually contain ink (not a blank line).
    let ink = b
        .pixels()
        .filter(|p| p.0[0] < 200 || p.0[1] < 200 || p.0[2] < 200)
        .count();
    assert!(
        ink > 300,
        "CJK text should render visible glyphs, got {ink}"
    );
}
