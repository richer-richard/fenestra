//! The painting and typography specimens in a window.
//! `cargo run --example specimen -- [dark] [type]`

use fenestra::shell::{WindowOptions, run_scene};
use fenestra::{Fonts, Theme};

fn main() {
    let dark = std::env::args().any(|a| a == "dark");
    let typography = std::env::args().any(|a| a == "type");
    let theme = if dark { Theme::dark() } else { Theme::light() };
    let bg = theme.bg;
    let mut fonts = Fonts::with_system();
    run_scene(
        WindowOptions::titled("fenestra specimen").with_size(760.0, 560.0),
        bg,
        move |scene, width, height, _bg| {
            let el = if typography {
                fenestra::kit::type_specimen::<()>(&theme)
            } else {
                fenestra::kit::specimen::<()>(&theme)
            };
            #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
            let built =
                fenestra::build_scene(&el, &theme, &mut fonts, (width as f32, height as f32));
            scene.append(&built, None);
        },
    )
    .expect("event loop failed");
}
