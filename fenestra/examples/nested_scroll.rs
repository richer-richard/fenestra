//! Nested scrolling with persistent offsets and auto-fading scrollbars.
//! `cargo run --example nested_scroll -- dark`

use fenestra::Theme;
use fenestra::shell::{WindowOptions, run_static};

fn main() {
    let dark = std::env::args().any(|a| a == "dark");
    let theme = if dark { Theme::dark() } else { Theme::light() };
    run_static(
        WindowOptions::titled("fenestra nested scroll").with_size(480.0, 520.0),
        theme,
        fenestra::kit::scroll_demo,
    )
    .expect("event loop failed");
}
