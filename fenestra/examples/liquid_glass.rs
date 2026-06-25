//! Liquid Glass showcase: the specular edge rim, directional body sheen, and
//! edge lensing (refraction) on a *vibrant* glass pane floating over
//! high-contrast content.
//!
//! The kit's stock [`Surface::Glass`](fenestra::Surface) is tuned legibility-
//! first (a fairly opaque 0.82 tint), which keeps text crisp but hides the
//! backdrop optics. This example composes a glassier pane — a low-alpha vibrancy
//! tint over a real backdrop blur — so the lensed rim and the way the straight
//! stripes *bend* through the rounded edge are plain to see. Every color comes
//! from theme tokens.
//!
//! `cargo run --example liquid_glass`

use fenestra::shell::render_element;
use fenestra::{
    AdaptiveTint, Color, Element, Material, Mode, SP2, SP3, SP4, SP6, ShadowToken, Sheen,
    SpecularEdge, TextSize, Theme, Weight, col, row, stack, text,
};

const SIZE: (u32, u32) = (760, 480);

/// Bold vertical stripes from the accent ramp — high-contrast content whose
/// straight edges visibly bend through the glass rim (the lensing pass).
fn striped_backdrop(t: &Theme) -> Element<()> {
    row().w_full().h_full().children(
        (0..19_usize)
            .map(|i| col().w(40.0).h_full().bg(t.accents.step(2 + i % 9)))
            .collect::<Vec<_>>(),
    )
}

/// A vibrant glass pane that shows the optics off: a translucent vibrancy tint,
/// a real backdrop blur + lensing, the directional specular rim, and the body
/// sheen.
fn glass_hero(t: &Theme) -> Element<()> {
    let chip = |label: &str, fill: Color| {
        row()
            .items_center()
            .px(SP3)
            .h(28.0)
            .rounded_full()
            .bg(fill)
            .child(
                text(label.to_owned())
                    .size(TextSize::Xs)
                    .weight(Weight::Semibold)
                    .color(t.text_on(fill)),
            )
    };
    // A glassier vibrancy tint than the stock Surface::Glass (0.82): a 0.5-alpha
    // tint of the raised surface so the lensed backdrop reads through it. The
    // Material levers (alpha, blur, OKLCH-chroma vibrancy) are the same ones
    // Surface::Glass uses — just dialed glassier for a hero surface.
    let fill = Material::new(0.5, 24.0, 1.6).tint(t.surface_raised);
    col()
        .absolute()
        .top(150.0)
        .left(180.0)
        .w(400.0)
        .p(SP6)
        .gap(SP4)
        .rounded(24.0)
        .bg(fill)
        .backdrop_blur(24.0)
        .specular_edge(SpecularEdge::glass())
        .sheen(Sheen::glass())
        .adaptive_tint(AdaptiveTint::glass())
        .border(1.0, t.border_subtle)
        .shadow(ShadowToken::Lg)
        .overflow_hidden()
        .children([
            text("Liquid Glass")
                .size(TextSize::Xl2)
                .weight(Weight::Semibold)
                .color(t.text),
            text("specular rim · body sheen · edge lensing")
                .size(TextSize::Sm)
                .color(t.text_muted),
            row().gap(SP2).children([
                chip("Danger", t.danger.solid),
                chip("Warning", t.warning.solid),
                chip("Success", t.success.solid),
            ]),
        ])
}

fn main() {
    let out = std::path::Path::new("liquid_glass");
    std::fs::create_dir_all(out).expect("create liquid_glass dir");
    for (mode, suffix) in [(Mode::Light, "light"), (Mode::Dark, "dark")] {
        let theme = Theme::from_accent(262.0, mode);
        let view = stack()
            .w_full()
            .h_full()
            .children([striped_backdrop(&theme), glass_hero(&theme)]);
        render_element(view, &theme, SIZE)
            .save(out.join(format!("glass_{suffix}.png")))
            .expect("write liquid_glass png");
    }
    println!("wrote liquid_glass/glass_{{light,dark}}.png");
}
