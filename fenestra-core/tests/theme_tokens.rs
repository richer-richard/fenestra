//! Locks every generated theme value and design token with text snapshots.
//! These numbers are the spec: a diff here is a design change, not a refactor.

use fenestra_core::{
    EASE_ACCELERATE, EASE_DECELERATE, EASE_EXIT, EASE_STANDARD, FOCUS_RING, MotionDuration,
    PRESS_SCALE, R_FULL, R_LG, R_MD, R_SM, R_XL, SP0, SP0_5, SP1, SP2, SP3, SP4, SP5, SP6, SP8,
    SP10, SP12, SP16, STATE_LAYER, TextSize, Theme, Weight,
};

#[test]
fn theme_light_tokens() {
    insta::assert_snapshot!(Theme::light().dump());
}

#[test]
fn theme_dark_tokens() {
    insta::assert_snapshot!(Theme::dark().dump());
}

#[test]
fn brand_color_constant_across_modes() {
    let light = Theme::light();
    let dark = Theme::dark();
    // Accent steps 9 and 10 are identical in both modes by design.
    for step in [9, 10] {
        assert_eq!(
            light.accents.step(step).to_rgba8(),
            dark.accents.step(step).to_rgba8(),
            "accent step {step} should be mode-invariant"
        );
    }
    // A9 has L 0.585 < 0.65, so text on accent is white in both modes.
    for theme in [&light, &dark] {
        assert_eq!(theme.on_accent.to_rgba8().r, 255);
        assert_eq!(theme.on_accent.to_rgba8().g, 255);
        assert_eq!(theme.on_accent.to_rgba8().b, 255);
    }
}

#[test]
fn static_tokens() {
    let spacing = [
        SP0, SP0_5, SP1, SP2, SP3, SP4, SP5, SP6, SP8, SP10, SP12, SP16,
    ];
    let mut out = String::new();
    out.push_str(&format!("spacing: {spacing:?}\n"));
    out.push_str(&format!(
        "radii: sm {R_SM} md {R_MD} lg {R_LG} xl {R_XL} full {R_FULL}\n"
    ));
    out.push_str("text (px / line-height / letter-spacing em):\n");
    for size in [
        TextSize::Xs,
        TextSize::Sm,
        TextSize::Base,
        TextSize::Lg,
        TextSize::Xl,
        TextSize::Xl2,
        TextSize::Xl3,
    ] {
        out.push_str(&format!(
            "  {size:?}: {} / {} / {}\n",
            size.px(),
            size.line_height(),
            size.letter_spacing()
        ));
    }
    out.push_str(&format!(
        "weights: regular {} medium {} semibold {}\n",
        Weight::Regular.value(),
        Weight::Medium.value(),
        Weight::Semibold.value()
    ));
    out.push_str(&format!(
        "motion: micro {} fast {} base {} slow {} (base exit {})\n",
        MotionDuration::Micro.ms(),
        MotionDuration::Fast.ms(),
        MotionDuration::Base.ms(),
        MotionDuration::Slow.ms(),
        MotionDuration::Base.exit_ms(),
    ));
    out.push_str(&format!(
        "easing standard: ({}, {}, {}, {})\n",
        EASE_STANDARD.x1, EASE_STANDARD.y1, EASE_STANDARD.x2, EASE_STANDARD.y2
    ));
    out.push_str(&format!(
        "easing decelerate: ({}, {}, {}, {})\n",
        EASE_DECELERATE.x1, EASE_DECELERATE.y1, EASE_DECELERATE.x2, EASE_DECELERATE.y2
    ));
    out.push_str(&format!(
        "easing accelerate: ({}, {}, {}, {})\n",
        EASE_ACCELERATE.x1, EASE_ACCELERATE.y1, EASE_ACCELERATE.x2, EASE_ACCELERATE.y2
    ));
    out.push_str(&format!(
        "easing exit: ({}, {}, {}, {})\n",
        EASE_EXIT.x1, EASE_EXIT.y1, EASE_EXIT.x2, EASE_EXIT.y2
    ));
    out.push_str(&format!(
        "focus ring: width {} offset {} alpha {}\n",
        FOCUS_RING.width, FOCUS_RING.offset, FOCUS_RING.alpha
    ));
    out.push_str(&format!("press scale: {PRESS_SCALE}\n"));
    out.push_str(&format!(
        "state layer: hover {} focus {} press {} drag {} (disabled container {} content {})\n",
        STATE_LAYER.hover,
        STATE_LAYER.focus,
        STATE_LAYER.press,
        STATE_LAYER.drag,
        STATE_LAYER.disabled_container,
        STATE_LAYER.disabled_content,
    ));
    insta::assert_snapshot!(out);
}
