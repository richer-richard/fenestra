//! M0 first pixel: a window with the theme background and one card-like
//! rounded rect with an Md shadow and a 1px border.

use fenestra::Theme;
use fenestra::shell::{WindowOptions, run_scene};

fn main() {
    let theme = Theme::light();
    let bg = theme.bg;
    run_scene(
        WindowOptions::titled("fenestra hello").with_size(800.0, 600.0),
        bg,
        move |scene, width, height, _bg| {
            fenestra::paint::paint_hello(scene, &theme, width, height);
        },
    )
    .expect("event loop failed");
}
