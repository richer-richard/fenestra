//! `CubicBezier::eval` must solve its x-curve to a documented tolerance
//! (|x(t*) − x| ≤ 1e-5) even on plateau curves whose x-derivative vanishes
//! mid-range.

use fenestra_anim::CubicBezier;
use proptest::prelude::*;

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

fn bezier_y_ref(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let t = bezier_t_for_x(x1, x2, x);
    let s = 1.0 - t;
    3.0 * s * s * t * y1 + 3.0 * s * t * t * y2 + t * t * t
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
        worst <= 1e-5,
        "identity-curve residual {worst} exceeds the documented bound"
    );
}

/// The same bound on the curves fenestra ships (Material tokens and the
/// motion presets), checked against a bisection reference.
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
            let y_ref = bezier_y_ref(x1, y1, x2, y2, x);
            let err = (curve.eval(x) - y_ref).abs();
            assert!(
                err <= 1e-5,
                "({x1},{y1},{x2},{y2}) at x={x}: eval residual {err}"
            );
        }
    }
}

proptest! {
    /// Across the whole valid control-point domain (x1, x2 in 0..=1, y1/y2
    /// allowed to overshoot), `eval` must stay within the documented 1e-5
    /// bound of the bisection reference — not just the curves fenestra
    /// happens to ship today.
    #[test]
    fn eval_matches_bisection_reference_for_any_valid_control_points(
        x1 in 0.0f32..=1.0,
        y1 in -2.0f32..=3.0,
        x2 in 0.0f32..=1.0,
        y2 in -2.0f32..=3.0,
        x in 0.001f32..0.999,
    ) {
        let curve = CubicBezier { x1, y1, x2, y2 };
        let y_ref = bezier_y_ref(x1, y1, x2, y2, x);
        let err = (curve.eval(x) - y_ref).abs();
        prop_assert!(
            err <= 1e-5,
            "({x1},{y1},{x2},{y2}) at x={x}: eval residual {err}"
        );
    }

    /// The boundary contract holds regardless of control points: `eval(0)`
    /// is exactly 0 and `eval(1)` is exactly 1.
    #[test]
    fn eval_is_exact_at_the_endpoints(
        x1 in 0.0f32..=1.0,
        y1 in -2.0f32..=3.0,
        x2 in 0.0f32..=1.0,
        y2 in -2.0f32..=3.0,
    ) {
        let curve = CubicBezier { x1, y1, x2, y2 };
        prop_assert_eq!(curve.eval(0.0), 0.0);
        prop_assert_eq!(curve.eval(1.0), 1.0);
    }
}
