//! `Style::ring` / `Element::ring`: a crisp outer ring rendered as a zero-blur
//! spread shadow (the "ring, not border" primitive).

use fenestra_core::{Color, Style};

#[test]
fn ring_pushes_a_zero_blur_spread_shadow() {
    let c = Color::new([0.1, 0.2, 0.3, 1.0]);
    let s = Style::default().ring(1.5, c);
    assert_eq!(s.shadows.len(), 1, "ring adds exactly one shadow layer");
    let r = s.shadows[0];
    // A ring is a zero-blur, positive-spread, centered shadow — a crisp band
    // around the box, not a soft drop.
    assert_eq!((r.dx, r.dy, r.blur, r.spread), (0.0, 0.0, 0.0, 1.5));
    assert_eq!(r.color, c);
}

#[test]
fn rings_stack_and_compose_with_existing_shadows() {
    let c = Color::new([0.0, 0.0, 0.0, 1.0]);
    // Two rings stack (e.g. a hairline + a selection ring); each appends.
    let s = Style::default().ring(1.0, c).ring(3.0, c);
    assert_eq!(s.shadows.len(), 2);
    assert_eq!(s.shadows[0].spread, 1.0);
    assert_eq!(s.shadows[1].spread, 3.0);
}
