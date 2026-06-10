//! A window with the theme background and one card with the signature
//! border-plus-shadow pairing, built from the element IR.

use fenestra::prelude::*;
use fenestra::shell::{WindowOptions, run_scene};

fn card<Msg>(theme: &Theme) -> Element<Msg> {
    col().items_center().justify_center().children([div()
        .w(320.0)
        .h(200.0)
        .bg(theme.surface_raised)
        .border(1.0, theme.border_subtle)
        .rounded(R_LG)
        .shadow(ShadowToken::Md)])
}

fn main() {
    let theme = Theme::light();
    let bg = theme.bg;
    let mut fonts = Fonts::with_system();
    run_scene(
        WindowOptions::titled("fenestra hello").with_size(800.0, 600.0),
        bg,
        move |scene, width, height, _bg| {
            #[expect(clippy::cast_possible_truncation, reason = "window sizes fit in f32")]
            let built = fenestra::build_scene(
                &card::<()>(&theme),
                &theme,
                &mut fonts,
                (width as f32, height as f32),
            );
            scene.append(&built, None);
        },
    )
    .expect("event loop failed");
}
