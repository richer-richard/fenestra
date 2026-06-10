//! The painting specimen in a window. Pass `dark` as an argument for the
//! dark theme: `cargo run --example specimen -- dark`.

use fenestra::Theme;
use fenestra::shell::{WindowOptions, run_scene};

fn main() {
    let dark = std::env::args().any(|a| a == "dark");
    let theme = if dark { Theme::dark() } else { Theme::light() };
    let bg = theme.bg;
    run_scene(
        WindowOptions::titled("fenestra specimen").with_size(760.0, 560.0),
        bg,
        move |scene, width, height, _bg| {
            let el = fenestra::kit::specimen::<()>(&theme);
            #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
            let built = fenestra::build_scene(&el, &theme, (width as f32, height as f32));
            scene.append(&built, None);
        },
    )
    .expect("event loop failed");
}
