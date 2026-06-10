//! M0 acceptance: the hello scene renders headlessly, with no window or
//! display server, to a non-uniform image of the requested size.

use fenestra_core::{Theme, paint::paint_hello};
use fenestra_shell::Headless;
use vello::Scene;

#[test]
fn hello_scene_renders_headless() {
    let theme = Theme::light();
    let mut scene = Scene::new();
    paint_hello(&mut scene, &theme, 800.0, 600.0);

    let mut headless = Headless::new().expect("headless renderer");
    let image = headless
        .render(&scene, 800, 600, theme.bg)
        .expect("headless render");

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
