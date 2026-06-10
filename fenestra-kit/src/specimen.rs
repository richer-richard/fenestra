//! The painting specimen: color ramps, the shadow stack, radii, gradients,
//! opacity, and clipping, all routed through theme tokens. Rendered as a
//! golden image in both modes; this is M1's visual regression corpus.

use fenestra_core::{
    Element, GradientStop, Paint, R_FULL, R_LG, R_MD, R_SM, R_XL, SP1, SP2, SP4, SP6, SP8,
    ShadowToken, Theme, col, div, row,
};

fn swatch_row<Msg>(colors: impl IntoIterator<Item = fenestra_core::Color>) -> Element<Msg> {
    row().gap(SP1).children(
        colors
            .into_iter()
            .map(|c| div().w(40.0).h(40.0).rounded(R_SM).bg(c)),
    )
}

/// Builds the full painting specimen for a theme.
pub fn specimen<Msg>(theme: &Theme) -> Element<Msg> {
    let ramp = |r: &fenestra_core::Ramp| (1..=12).map(|i| r.step(i)).collect::<Vec<_>>();
    let status = |s: &fenestra_core::StatusColors| [s.bg, s.border, s.solid, s.text];

    let shadow_card = |token: ShadowToken| {
        div()
            .w(96.0)
            .h(64.0)
            .rounded(R_LG)
            .bg(theme.surface_raised)
            .border(1.0, theme.border_subtle)
            .shadow(token)
    };

    let radius_box = |r: f32| {
        div()
            .w(64.0)
            .h(64.0)
            .rounded(r)
            .bg(theme.accent_bg)
            .border(1.0, theme.accent_border)
    };

    let linear = Paint::LinearGradient {
        angle_deg: 135.0,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: theme.accents.step(7),
            },
            GradientStop {
                offset: 1.0,
                color: theme.accents.step(10),
            },
        ],
    };
    let radial = Paint::RadialGradient {
        center: (0.3, 0.3),
        radius: 1.2,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: theme.accents.step(4),
            },
            GradientStop {
                offset: 1.0,
                color: theme.accents.step(9),
            },
        ],
    };

    col().p(SP6).gap(SP6).bg(theme.bg).children([
        // 12-step ramps.
        swatch_row(ramp(&theme.neutrals)),
        swatch_row(ramp(&theme.accents)),
        // Status sets: bg / border / solid / text for each hue.
        row().gap(SP8).children([
            swatch_row(status(&theme.danger)),
            swatch_row(status(&theme.warning)),
            swatch_row(status(&theme.success)),
        ]),
        // The shadow stack on raised cards (the signature pairing of a
        // subtle border with a soft shadow).
        row().gap(SP8).p(SP2).children([
            shadow_card(ShadowToken::Xs),
            shadow_card(ShadowToken::Sm),
            shadow_card(ShadowToken::Md),
            shadow_card(ShadowToken::Lg),
        ]),
        // The radius scale.
        row().gap(SP4).children([
            radius_box(R_SM),
            radius_box(R_MD),
            radius_box(R_LG),
            radius_box(R_XL),
            radius_box(R_FULL),
        ]),
        // Gradients, opacity, and rounded clipping: the clipped box has
        // an oversized gradient child that must not bleed past R_LG.
        row().gap(SP4).children([
            div().w(160.0).h(64.0).rounded(R_MD).bg(linear),
            div().w(160.0).h(64.0).rounded(R_MD).bg(radial.clone()),
            div()
                .w(160.0)
                .h(64.0)
                .rounded(R_MD)
                .bg(theme.accent)
                .opacity(0.5),
            div()
                .w(160.0)
                .h(64.0)
                .rounded(R_LG)
                .overflow_hidden()
                .children([div().w(240.0).h(120.0).shrink0().bg(radial)]),
        ]),
    ])
}
