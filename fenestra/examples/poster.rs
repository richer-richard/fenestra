//! Renders the editorial poster headlessly to `gallery/poster.png` — the
//! "this is a native Rust app?" shot. Custom faces (Playfair Display, OFL)
//! register under the Display/Serif roles; the deep-green field and ochre
//! accents come from `Theme::duotone`; every color routes through tokens.
//!
//! `cargo run --example poster`

use fenestra::prelude::*;
use fenestra::shell::render_element_with;

fn main() {
    let mut fonts = Fonts::embedded();
    assert!(fonts.register(
        FamilyRole::Display,
        include_bytes!("assets/poster/PlayfairDisplay.ttf").to_vec(),
    ));
    assert!(fonts.register(
        FamilyRole::Serif,
        include_bytes!("assets/poster/PlayfairDisplay-Italic.ttf").to_vec(),
    ));
    let theme = Theme::duotone(152.0, 6.0, 72.0, Mode::Dark);
    let image = render_element_with(poster::<()>(&theme), &theme, (1040, 1300), &mut fonts);
    std::fs::create_dir_all("gallery").expect("create gallery dir");
    image.save("gallery/poster.png").expect("write png");
    println!("wrote gallery/poster.png");
}
