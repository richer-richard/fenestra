//! The editorial poster: custom display faces (Playfair Display, OFL),
//! a deep-green duotone field, ochre accents, botanical path art — every
//! color through theme tokens. Opens in a window by default;
//! `cargo run --example poster -- shot` renders gallery/poster.png
//! headlessly instead (the README art).

use fenestra::prelude::*;
use fenestra::shell::{render_element_with, run_static};

const DISPLAY: &[u8] = include_bytes!("assets/poster/PlayfairDisplay.ttf");
const ITALIC: &[u8] = include_bytes!("assets/poster/PlayfairDisplay-Italic.ttf");

fn theme() -> Theme {
    Theme::duotone(152.0, 6.0, 72.0, Mode::Dark)
}

fn main() {
    if std::env::args().any(|a| a == "shot") {
        let mut fonts = Fonts::embedded();
        assert!(fonts.register(FamilyRole::Display, DISPLAY.to_vec()));
        assert!(fonts.register(FamilyRole::Serif, ITALIC.to_vec()));
        let theme = theme();
        let image = render_element_with(poster::<()>(&theme), &theme, (1040, 1300), &mut fonts);
        std::fs::create_dir_all("gallery").expect("create gallery dir");
        image.save("gallery/poster.png").expect("write png");
        println!("wrote gallery/poster.png");
        return;
    }
    // Windowed: the fixed 1040x1300 canvas scrolls inside the window.
    run_static(
        WindowOptions::titled("fenestra poster")
            .with_size(1040.0, 860.0)
            .with_min_size(480.0, 360.0)
            .with_font(FamilyRole::Display, DISPLAY.to_vec())
            .with_font(FamilyRole::Serif, ITALIC.to_vec()),
        theme(),
        |t| {
            col()
                .w_full()
                .h_full()
                .scroll_y()
                .id("poster-scroll")
                .children([div().w(1040.0).h(1300.0).shrink0().children([poster(t)])])
        },
    )
    .expect("event loop");
}
