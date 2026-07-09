//! OKLCH color picker: the accessibility projection (every channel exposes
//! `Semantics::Slider` with the right label/value/bounds, the whole widget
//! carries a group label, the swatch reports the current color) and the
//! hostile-input/parsing contract at the public API boundary.

use fenestra_core::{
    AccessNode, Color, Element, Fonts, FrameState, Semantics, Theme, build_frame, col, oklch,
};
use fenestra_kit::{color_picker, format_color_text, parse_color_text};

fn find<'a>(node: &'a AccessNode, pred: &impl Fn(&AccessNode) -> bool) -> Option<&'a AccessNode> {
    if pred(node) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, pred))
}

fn find_all<'a>(
    node: &'a AccessNode,
    pred: &impl Fn(&AccessNode) -> bool,
    out: &mut Vec<&'a AccessNode>,
) {
    if pred(node) {
        out.push(node);
    }
    for c in &node.children {
        find_all(c, pred, out);
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Changed(Color),
    Typed(String, Option<Color>),
}

#[test]
fn every_channel_exposes_slider_semantics_with_label_and_bounds() {
    let value = oklch(0.6, 0.12, 250.0);
    let view: Element<Msg> = col().children([color_picker(value)
        .label("Accent")
        .on_change(Msg::Changed)
        .on_text_change(Msg::Typed)]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (600.0, 400.0), 1.0);
    let tree = frame.access_tree();

    let mut sliders = Vec::new();
    find_all(
        &tree,
        &|n| matches!(n.semantics, Some(Semantics::Slider { .. })),
        &mut sliders,
    );
    let by_label = |label: &str| {
        sliders
            .iter()
            .find(|n| n.label.as_deref() == Some(label))
            .unwrap_or_else(|| panic!("no slider labeled {label:?}; got {sliders:#?}"))
    };

    let lightness = by_label("Lightness");
    assert!(lightness.focusable, "Lightness is keyboard focusable");
    assert!(
        matches!(lightness.semantics, Some(Semantics::Slider { min, max, .. }) if min == 0.0 && max == 1.0)
    );

    let chroma = by_label("Chroma");
    assert!(chroma.focusable, "Chroma is keyboard focusable");
    assert!(
        matches!(chroma.semantics, Some(Semantics::Slider { min, max, .. }) if min == 0.0 && (max - fenestra_kit::MAX_CHROMA).abs() < 1e-6)
    );

    let hue = by_label("Hue");
    assert!(hue.focusable, "Hue is keyboard focusable");
    assert!(matches!(
        hue.semantics,
        Some(Semantics::Slider { value, min, max }) if (value - 250.0).abs() < 1e-3 && min == 0.0 && max == 360.0
    ));

    let alpha = by_label("Alpha");
    assert!(alpha.focusable, "Alpha is keyboard focusable");
    assert!(matches!(
        alpha.semantics,
        Some(Semantics::Slider { value, min, max }) if (value - 1.0).abs() < 1e-3 && min == 0.0 && max == 1.0
    ));

    // The whole widget is grouped under its own accessible label.
    assert!(
        find(&tree, &|n| n.label.as_deref() == Some("Accent")).is_some(),
        "the widget's outer group carries the accessible label"
    );

    // The swatch reports the current color as an image with its hex in the label
    // (every generated texture in the widget auto-projects `Semantics::Image`,
    // so match on the label rather than the role to find the swatch itself).
    let want_label = format!("Current color {}", format_color_text(value));
    let swatch = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Image))
            && n.label.as_deref() == Some(want_label.as_str())
    })
    .expect("the swatch exposes an image role with the current color in its label");
    assert_eq!(swatch.label.as_deref(), Some(want_label.as_str()));
}

#[test]
fn disabled_picker_has_no_focusable_channels() {
    let view: Element<Msg> = col().children([color_picker(oklch(0.6, 0.1, 30.0))
        .label("Accent")
        .disabled(true)
        .on_change(Msg::Changed)]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (600.0, 400.0), 1.0);
    let tree = frame.access_tree();

    let mut sliders = Vec::new();
    find_all(
        &tree,
        &|n| matches!(n.semantics, Some(Semantics::Slider { .. })),
        &mut sliders,
    );
    assert!(
        sliders.iter().all(|n| !n.focusable),
        "a disabled picker exposes no focusable channel: {sliders:#?}"
    );
}

#[test]
fn gamut_edge_point_shows_the_indicator_label() {
    // Near-white with high chroma: sits on the sRGB gamut edge at any hue.
    let extreme = oklch(0.97, 0.35, 150.0);
    let view: Element<Msg> = col().children([color_picker(extreme).on_change(Msg::Changed)]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (600.0, 400.0), 1.0);
    let tree = frame.access_tree();

    assert!(
        find(&tree, &|n| n
            .label
            .as_deref()
            .is_some_and(|l| l.contains("gamut edge")))
        .is_some(),
        "a point on the gamut edge surfaces the gamut-edge indicator label"
    );
}

#[test]
fn hostile_text_never_panics_and_never_commits() {
    for bad in [
        "",
        "not a color",
        "oklch(nan 0 0)",
        "#zz",
        "🎨",
        "oklch(1 1 1 1 1)",
    ] {
        assert!(parse_color_text(bad).is_none(), "expected None for {bad:?}");
    }
    // A valid parse round-trips through the public API without panicking.
    assert!(parse_color_text("#336699").is_some());
    assert!(parse_color_text("oklch(0.5 0.1 200)").is_some());
}
