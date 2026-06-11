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
/// hitting wgpu's fatal validation handler. GPU-free: the clamp contract
/// itself, on every platform.
#[test]
fn clamp_size_contract() {
    fenestra_shell::with_headless(|h| {
        let max = h.max_dimension();
        assert!(max >= 1024, "real devices support >= 1024, got {max}");
        assert_eq!(h.clamp_size(0, 0), (1, 1));
        assert_eq!(h.clamp_size(1_000_000, 8), (max, 8));
        assert_eq!(h.clamp_size(640, 480), (640, 480));
    })
    .expect("headless renderer unavailable");
}

/// The clamped maximum-width render end to end. WARP (Windows' software
/// DX12 rasterizer) access-violates inside the driver on max-dimension
/// renders, so this runs where rasterizers survive it; the contract above
/// is covered everywhere.
#[cfg_attr(
    target_os = "windows",
    ignore = "WARP crashes (STATUS_ACCESS_VIOLATION) rendering at max texture width"
)]
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
