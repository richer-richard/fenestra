//! Renders all six `fenestra-looks` design languages to `looks/*.png`, each
//! with its own bundled fonts — the one place that shows the framework's
//! aesthetic range (`product`, `editorial`, `terminal`, `console`,
//! `warm_editorial`, `playful`) in a single run.
//!
//! A look is one call: `let look = fenestra_looks::editorial(mode);` gives you
//! a ready `look.theme` and `look.fonts()`; in a windowed app, register the
//! same faces via `WindowOptions::with_font`.
//!
//! `cargo run --example looks`

use fenestra::Mode;
use fenestra::shell::render_element_with;

fn main() {
    let out = std::path::Path::new("looks");
    std::fs::create_dir_all(out).expect("create looks dir");

    for (mode, suffix) in [(Mode::Light, "light"), (Mode::Dark, "dark")] {
        for look in fenestra_looks::all(mode) {
            // Each look ships its own faces; render headlessly with them so the
            // typography reads as intended (not the stock Inter).
            let mut fonts = look.fonts();
            let panel = fenestra::kit::gallery_controls(&look.theme);
            render_element_with(panel, &look.theme, (688, 900), &mut fonts)
                .save(out.join(format!("{}_{suffix}.png", look.name)))
                .expect("write look png");
        }
        println!(
            "wrote looks/{{product,editorial,terminal,console,warm_editorial,playful}}_{suffix}.png"
        );
    }
}
