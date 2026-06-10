//! The holy-grail layout: grid app shell with header, footer, two fixed
//! sidebars, and fluid main content. `cargo run --example holy_grail -- dark`

use fenestra::Theme;
use fenestra::shell::{WindowOptions, run_static};

fn main() {
    let dark = std::env::args().any(|a| a == "dark");
    let theme = if dark { Theme::dark() } else { Theme::light() };
    run_static(
        WindowOptions::titled("fenestra holy grail").with_size(720.0, 480.0),
        theme,
        fenestra::kit::holy_grail,
    )
    .expect("event loop failed");
}
