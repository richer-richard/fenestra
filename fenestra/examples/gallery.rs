//! Renders every kit widget in every state to `gallery/*.png`, headlessly —
//! no window or display server. These images are the visual regression
//! corpus and the README art.
//!
//! `cargo run --example gallery`

use fenestra::shell::render_element;
use fenestra::{BaseField, Contrast, Elevation, Mode, RadiusScale, Theme};

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

        // The sharp/minimal "console" look — design range beyond the soft default.
        let console_theme = Theme::derive(
            BaseField {
                hue: 250.0,
                chroma: 1.5,
            },
            130.0,
            Contrast::High,
            mode,
        )
        .with_radius(RadiusScale::sharp())
        .with_elevation(Elevation::Flat);
        let console = render_element(
            fenestra::kit::console_showcase(&console_theme),
            &console_theme,
            (1200, 760),
        );
        console
            .save(out.join(format!("console_{suffix}.png")))
            .expect("write console");

        println!("wrote gallery/{{controls,display,console}}_{suffix}.png");
    }
}
