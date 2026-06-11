//! Renders every kit widget in every state to `gallery/*.png`, headlessly —
//! no window or display server. These images are the visual regression
//! corpus and the README art.
//!
//! `cargo run --example gallery`

use fenestra::shell::render_element;
use fenestra::{Mode, Theme};

fn main() {
    let out = std::path::Path::new("gallery");
    std::fs::create_dir_all(out).expect("create gallery dir");

    for (mode, suffix) in [(Mode::Light, "light"), (Mode::Dark, "dark")] {
        let theme = Theme::from_accent(262.0, mode);
        let controls = render_element(fenestra::kit::gallery_controls(&theme), &theme, (688, 900));
        controls
            .save(out.join(format!("controls_{suffix}.png")))
            .expect("write controls");

        let display = render_element(fenestra::kit::gallery_display(&theme), &theme, (760, 1190));
        display
            .save(out.join(format!("display_{suffix}.png")))
            .expect("write display");

        println!("wrote gallery/controls_{suffix}.png and gallery/display_{suffix}.png");
    }
}
