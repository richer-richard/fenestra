//! Renders every kit widget in every state to `gallery/*.png`, headlessly —
//! no window or display server. These images are the visual regression
//! corpus and the README art.
//!
//! `cargo run --example gallery`

use fenestra::shell::{render_element, render_element_with};
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

        let feedback = render_element(fenestra::kit::gallery_feedback(&theme), &theme, (688, 820));
        feedback
            .save(out.join(format!("feedback_{suffix}.png")))
            .expect("write feedback");

        // The sharp/minimal "console" look — straight from fenestra-looks (with
        // its own faces), instead of re-deriving the palette by hand.
        let console = fenestra_looks::console(mode);
        let mut console_fonts = console.fonts();
        render_element_with(
            fenestra::kit::console_showcase(&console.theme),
            &console.theme,
            (1200, 760),
            &mut console_fonts,
        )
        .save(out.join(format!("console_{suffix}.png")))
        .expect("write console");

        println!("wrote gallery/{{controls,display,feedback,console}}_{suffix}.png");
    }
}
