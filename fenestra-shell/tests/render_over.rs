//! `render_element_over`: the offline-sampler entry point — caller-supplied
//! base color (transparent backgrounds render real alpha) and scale factor
//! (cheap preview frames), sharing the same two-pass pipeline as every other
//! headless render.

use fenestra_core::{Color, Element, Fonts, FrameState, Theme, div, stack};
use fenestra_shell::render_element_over;

fn scene() -> Element<()> {
    // A 100×40 opaque box centered on an otherwise empty canvas.
    stack().w(200.0).h(100.0).children([div()
        .w_full()
        .h_full()
        .items_center()
        .justify_center()
        .child(div().w(100.0).h(40.0).bg(Color::new([1.0, 0.0, 0.0, 1.0])))])
}

fn render(bg: Color, scale: f64) -> image::RgbaImage {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    render_element_over(
        scene(),
        &Theme::light(),
        (200, 100),
        scale,
        bg,
        &mut fonts,
        &mut state,
    )
    .expect("headless render")
}

#[test]
fn transparent_base_color_reaches_the_pixels() {
    let img = render(Color::TRANSPARENT, 1.0);
    assert_eq!(img.dimensions(), (200, 100));
    // A corner far from the box: fully transparent.
    assert_eq!(img.get_pixel(2, 2).0[3], 0, "empty canvas keeps alpha 0");
    // The box center: opaque red.
    let center = img.get_pixel(100, 50).0;
    assert_eq!(center[3], 255, "the box is opaque");
    assert!(center[0] > 200, "and red: {center:?}");
}

#[test]
fn scale_factor_multiplies_the_pixel_size() {
    let img = render(Color::TRANSPARENT, 0.5);
    assert_eq!(img.dimensions(), (100, 50), "0.5 scale halves the texture");
    // The box still covers the center at the scaled location.
    let center = img.get_pixel(50, 25).0;
    assert_eq!(center[3], 255);
    assert!(center[0] > 200, "red at the scaled center: {center:?}");
}

#[test]
fn opaque_base_color_flattens_the_canvas() {
    let img = render(Color::new([0.0, 0.0, 0.0, 1.0]), 1.0);
    let corner = img.get_pixel(2, 2).0;
    assert_eq!(corner, [0, 0, 0, 255], "the base color fills empty space");
}
