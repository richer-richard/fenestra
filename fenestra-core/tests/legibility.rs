//! `Frame::legibility` and the WCAG 2 helpers measure real resolved colors —
//! the per-node evidence behind "prove this UI is legible".

use fenestra_core::{
    Color, Element, Fonts, Frame, FrameState, Semantics, Theme, build_frame, col, lc_abs,
    linear_gradient, oklch, text, wcag2_ratio,
};

fn frame_of(el: &Element<()>, theme: &Theme) -> Frame {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    build_frame(el, theme, &mut fonts, &mut state, (400.0, 200.0), 1.0)
}

#[test]
fn wcag2_ratio_black_on_white_is_21() {
    let black = Color::from_rgba8(0, 0, 0, 255);
    let white = Color::from_rgba8(255, 255, 255, 255);
    let r = wcag2_ratio(black, white);
    assert!(
        (r - 21.0).abs() < 0.05,
        "black on white should be 21.0, got {r}"
    );
    // Symmetric in its arguments.
    assert!((wcag2_ratio(white, black) - 21.0).abs() < 0.05);
    // Identical colors → ratio 1.
    assert!((wcag2_ratio(white, white) - 1.0).abs() < 1e-6);
}

#[test]
fn legibility_reports_text_on_its_fill() {
    let theme = Theme::light();
    let view: Element<()> = col()
        .bg(theme.surface)
        .p(16.0)
        .children([text("Legible body")]);
    let frame = frame_of(&view, &theme);
    let report = frame.legibility(theme.bg);
    assert_eq!(report.len(), 1, "one text node, got {report:?}");
    let item = &report[0];
    assert_eq!(item.text, "Legible body");
    assert_eq!(item.bg, theme.surface, "background is the col's fill");
    // The reported lc matches the standalone APCA computation.
    assert!((item.lc - lc_abs(item.fg, item.bg)).abs() < 0.01);
    // The theme is provably legible, so its body text clears the APCA floor.
    assert!(
        item.passes_apca,
        "theme body text should clear APCA: {item:?}"
    );
}

#[test]
fn legibility_flags_low_contrast() {
    let theme = Theme::light();
    // Near-invisible: very light text on a slightly lighter background.
    let faint = oklch(0.95, 0.0, 0.0);
    let bg = oklch(0.97, 0.0, 0.0);
    let view: Element<()> = col().bg(bg).children([text("Hard to read").color(faint)]);
    let frame = frame_of(&view, &theme);
    let report = frame.legibility(theme.bg);
    assert_eq!(report.len(), 1);
    assert!(
        !report[0].passes_apca,
        "low-contrast text should fail APCA: {:?}",
        report[0]
    );
    assert!(
        !report[0].passes_wcag2,
        "low-contrast text should fail WCAG 2: {:?}",
        report[0]
    );
}

#[test]
fn legibility_measures_gradient_background_not_solid_ancestor() {
    // A text node over a GRADIENT fill: the effective background is the gradient
    // itself, not the nearest *solid* ancestor. White text on a light gradient is
    // illegible even though it would clear contrast against a dark window bg — the
    // check must measure the gradient (worst-contrast stop), never fall through it.
    // Before the fix, `solid_fill` returned None for gradients, so the check used
    // the window bg → a silent FALSE PASS for any text on an authored gradient.
    let theme = Theme::light();
    let white = Color::from_rgba8(255, 255, 255, 255);
    // A genuinely light gradient (two distinct light colors → stays a gradient).
    let light_grad = linear_gradient(
        0.0,
        [
            Color::from_rgba8(255, 255, 255, 255),
            Color::from_rgba8(224, 224, 224, 255),
        ],
    );
    let view: Element<()> = col()
        .bg(light_grad)
        .children([text("Invisible on light").color(white)]);
    let frame = frame_of(&view, &theme);
    // Window bg is BLACK: the buggy fall-through would report enormous contrast.
    let report = frame.legibility(Color::from_rgba8(0, 0, 0, 255));
    assert_eq!(report.len(), 1, "one text node, got {report:?}");
    let item = &report[0];
    assert!(
        !item.bg_uniform,
        "a gradient background is not a uniform color: {item:?}"
    );
    assert!(
        !item.passes_apca,
        "white text on a light gradient must FAIL APCA when measured against the \
         gradient (not the black window bg): {item:?}"
    );
}

#[test]
fn aria_role_is_public() {
    assert_eq!(Semantics::Button.aria_role(), "button");
    assert_eq!(
        Semantics::TextInput { multiline: false }.aria_role(),
        "textbox"
    );
}
