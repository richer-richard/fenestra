//! Paint-time transforms honor `scale_xy` / `transform_origin`. The
//! `CubicBezier`/`SpringSpec` accuracy tests these transform tests used to
//! sit alongside moved to `fenestra-anim` with the types themselves — see
//! that crate's `tests/bezier.rs` and `tests/spring.rs`.

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame, by, col, div};
use kurbo::Point;

/// Non-uniform paint-time scale: `.scale_xy(2, 1)` doubles the painted width
/// without touching height, and hit-testing follows the paint.
#[test]
fn scale_xy_paints_and_hit_tests_nonuniformly() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let view: Element<()> = col().w(400.0).h(200.0).children([div()
        .id("box")
        .w(100.0)
        .h(40.0)
        .scale_xy(2.0, 1.0)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 200.0), 1.0);

    let id = frame.get(&by::id("box")).id;
    let c = frame.rect_of(id).expect("box rect").center();

    // 75px right of center: outside the 50px layout half-width, inside the
    // 100px painted half-width.
    let stretched = Point::new(c.x + 75.0, c.y);
    assert!(
        frame.hit_chain(stretched).contains(&id),
        "x-scaled element hit-tests at its painted width"
    );
    // 30px below center: outside the 20px half-height, which scale_xy(2, 1)
    // must NOT stretch.
    let below = Point::new(c.x, c.y + 30.0);
    assert!(
        !frame.hit_chain(below).contains(&id),
        "y stays unscaled under scale_xy(2, 1)"
    );
}

/// `transform_origin` moves the pivot: rotating 90° about the top-left corner
/// paints the box beside its layout slot, and hit-testing follows.
#[test]
fn transform_origin_pivots_rotation() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;

    let view: Element<()> = col().w(400.0).h(400.0).p(150.0).children([div()
        .id("card")
        .w(100.0)
        .h(40.0)
        .rotate(90.0)
        .transform_origin(0.0, 0.0)]);
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 400.0), 1.0);

    let id = frame.get(&by::id("card")).id;
    let rect = frame.rect_of(id).expect("card rect");
    let (tlx, tly) = (rect.x0, rect.y0);

    // kurbo's 90° rotation maps a local offset (x, y) from the pivot to
    // (−y, x): the rect center (50, 20) lands at (−20, 50) from the top-left.
    let painted_center = Point::new(tlx - 20.0, tly + 50.0);
    assert!(
        frame.hit_chain(painted_center).contains(&id),
        "rotation pivots about the top-left origin"
    );
    let layout_center = rect.center();
    assert!(
        !frame.hit_chain(layout_center).contains(&id),
        "the layout slot no longer contains the rotated box"
    );
}
