//! Hardening: the headless API must accept hostile sizes without panicking —
//! it is the API agents call with generated values.

use fenestra_core::{Element, Theme, col, text};
use fenestra_shell::render_element;

/// A zero-dimension request renders a 1x1 image instead of panicking.
#[test]
fn zero_size_render_clamps_to_one_pixel() {
    let el: Element<()> = col().children([text("x")]);
    let image = render_element(el, &Theme::light(), (0, 0));
    assert_eq!(image.dimensions(), (1, 1));
}

/// A request beyond the device texture limit clamps to the limit instead of
/// hitting wgpu's fatal validation handler.
#[test]
fn oversized_render_clamps_to_device_limit() {
    let el: Element<()> = col();
    let image = render_element(el, &Theme::light(), (1_000_000, 8));
    assert!(
        image.width() >= 1024,
        "clamped width should still be a usable texture size, got {}",
        image.width()
    );
    assert!(
        image.width() < 1_000_000,
        "width must clamp below the request"
    );
    assert_eq!(image.height(), 8);
}
