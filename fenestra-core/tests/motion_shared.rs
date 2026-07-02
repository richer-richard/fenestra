//! The shared animation-math seams that `fenestra-motion` samples offline and
//! the interactive transition engine samples per frame: `CubicBezier::eval`
//! must solve its x-curve to a documented tolerance even on plateau curves,
//! `SpringSpec::step` is the public closed-form spring (with initial
//! velocity), and paint-time transforms honor `scale_xy` / `transform_origin`.

use fenestra_core::{
    CubicBezier, Element, Fonts, FrameState, SpringSpec, Theme, build_frame, by, col, div,
};
use kurbo::Point;

/// Solves x(t) = u by bisection to near-f32 precision: the reference the
/// production Newton solver is measured against.
fn bezier_t_for_x(x1: f32, x2: f32, u: f32) -> f32 {
    let bez = |t: f32, p1: f32, p2: f32| {
        let s = 1.0 - t;
        3.0 * s * s * t * p1 + 3.0 * s * t * t * p2 + t * t * t
    };
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < u {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// `eval` promises |x(t*) − x| ≤ 1e-5. On a curve whose y control points
/// equal its x control points, y(t) == x(t), so `eval(x)` must return `x`
/// itself — including on (1, 1, 0, 0), whose x-derivative vanishes at
/// t = 0.5 (the plateau that stalls plain Newton).
#[test]
fn bezier_eval_is_accurate_on_plateau_curves() {
    let plateau = CubicBezier {
        x1: 1.0,
        y1: 1.0,
        x2: 0.0,
        y2: 0.0,
    };
    let mut worst = 0.0f32;
    for i in 1..1000 {
        let x = i as f32 / 1000.0;
        let err = (plateau.eval(x) - x).abs();
        worst = worst.max(err);
    }
    assert!(
        worst <= 2e-5,
        "identity-curve residual {worst} exceeds the documented bound"
    );
}

/// The same bound on the curves fenestra actually ships (Material tokens and
/// the motion presets), checked against a bisection reference.
#[test]
fn bezier_eval_matches_bisection_reference_on_shipped_curves() {
    let curves = [
        (0.2, 0.0, 0.0, 1.0),    // EASE_STANDARD
        (0.0, 0.0, 0.2, 1.0),    // EASE_DECELERATE
        (0.4, 0.0, 1.0, 1.0),    // EASE_ACCELERATE
        (0.65, 0.0, 0.35, 1.0),  // EASE_IN_OUT_CUBIC
        (0.16, 1.0, 0.3, 1.0),   // crisp entrance
        (0.45, 0.0, 0.55, 1.0),  // editorial in-out
        (0.34, 1.56, 0.64, 1.0), // overshoot pop (y > 1)
    ];
    for (x1, y1, x2, y2) in curves {
        let curve = CubicBezier { x1, y1, x2, y2 };
        for i in 1..500 {
            let x = i as f32 / 500.0;
            let t = bezier_t_for_x(x1, x2, x);
            let s = 1.0 - t;
            let y_ref = 3.0 * s * s * t * y1 + 3.0 * s * t * t * y2 + t * t * t;
            let err = (curve.eval(x) - y_ref).abs();
            assert!(
                err <= 2e-5,
                "({x1},{y1},{x2},{y2}) at x={x}: eval residual {err}"
            );
        }
    }
}

/// The closed-form spring step response: starts at zero, settles to exactly
/// 1.0, and is monotone when critically damped.
#[test]
fn spring_step_starts_at_zero_and_settles_to_one() {
    let spring = SpringSpec {
        stiffness: 380.0,
        damping: 26.0,
    };
    let (start, settled_at_start) = spring.step(0.0, 0.0);
    assert_eq!(start, 0.0);
    assert!(!settled_at_start);

    let (end, settled) = spring.step(0.0, 30.0);
    assert_eq!(end, 1.0, "a settled spring reports exactly 1.0");
    assert!(settled);

    // Critical damping (damping = 2√stiffness): monotone, never overshoots.
    let critical = SpringSpec {
        stiffness: 100.0,
        damping: 20.0,
    };
    let mut prev = 0.0;
    for i in 0..=200 {
        let t = i as f32 * 0.01;
        let (x, _) = critical.step(0.0, t);
        assert!(x >= prev - 1e-6, "critical spring dipped at t={t}");
        assert!(x <= 1.0 + 1e-6, "critical spring overshot at t={t}");
        prev = x;
    }
}

/// An underdamped spring overshoots its target and the overshoot stays within
/// the decay envelope.
#[test]
fn underdamped_spring_overshoots_within_envelope() {
    let spring = SpringSpec {
        stiffness: 380.0,
        damping: 12.0,
    };
    let omega = spring.stiffness.sqrt();
    let zeta = spring.damping / (2.0 * omega);
    // x(t) = 1 − e^{−ζωt}(cos(ω_d t) + (ζω/ω_d)·sin(ω_d t)): the sinusoid's
    // amplitude is √(1 + (ζω/ω_d)²), and that factor scales the decay
    // envelope bounding |x − 1|.
    let wd = omega * (1.0 - zeta * zeta).sqrt();
    let amplitude = (1.0 + (zeta * omega / wd).powi(2)).sqrt();
    let mut peak = 0.0f32;
    for i in 1..400 {
        let t = i as f32 * 0.005;
        let (x, _) = spring.step(0.0, t);
        peak = peak.max(x);
        let envelope = amplitude * (-zeta * omega * t).exp();
        assert!(
            x <= 1.0 + envelope + 1e-4,
            "overshoot at t={t} exceeds the decay envelope"
        );
    }
    assert!(
        peak > 1.01,
        "underdamped spring never overshot (peak {peak})"
    );
}

/// Initial velocity shapes the launch: positive velocity runs ahead of the
/// zero-velocity response early on, negative velocity dips behind it, and
/// both settle to the same target.
#[test]
fn spring_initial_velocity_shapes_the_launch() {
    let spring = SpringSpec {
        stiffness: 170.0,
        damping: 26.0,
    };
    let t_early = 0.05;
    let (rest, _) = spring.step(0.0, t_early);
    let (launched, _) = spring.step(8.0, t_early);
    let (dragged, _) = spring.step(-8.0, t_early);
    assert!(
        launched > rest + 1e-3,
        "positive v0 should lead: {launched} vs {rest}"
    );
    assert!(
        dragged < rest - 1e-3,
        "negative v0 should lag: {dragged} vs {rest}"
    );

    let (a, sa) = spring.step(8.0, 30.0);
    let (b, sb) = spring.step(-8.0, 30.0);
    assert_eq!(a, 1.0);
    assert_eq!(b, 1.0);
    assert!(sa && sb);
}

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
