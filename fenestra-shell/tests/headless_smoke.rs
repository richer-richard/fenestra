//! M0 acceptance: an element tree renders headlessly, with no window or
//! display server, to a non-uniform image of the requested size.

use fenestra_core::{R_LG, ShadowToken, Theme, col, div};
use fenestra_shell::render_element;

#[test]
fn hello_scene_renders_headless() {
    let theme = Theme::light();
    let card = col().items_center().justify_center().children([div()
        .w(320.0)
        .h(200.0)
        .bg(theme.surface_raised)
        .border(1.0, theme.border_subtle)
        .rounded(R_LG)
        .shadow(ShadowToken::Md)]);

    let image = render_element::<()>(card, &theme, (800, 600));

    assert_eq!(image.width(), 800);
    assert_eq!(image.height(), 600);

    // Non-uniform: the card, its border, and its shadow must differ from bg.
    let first = image.get_pixel(0, 0);
    assert!(
        image.pixels().any(|p| p != first),
        "rendered image is uniform; nothing was painted"
    );

    // The card center is the raised surface (pure white in the light theme).
    let center = image.get_pixel(400, 300);
    assert_eq!(center.0, [255, 255, 255, 255], "card fill should be white");

    // Debugging aid: dump the render for human/agent inspection.
    if let Ok(path) = std::env::var("FENESTRA_WRITE_PNG") {
        image.save(&path).expect("failed to write debug PNG");
    }
}
