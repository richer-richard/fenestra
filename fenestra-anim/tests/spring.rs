//! `SpringSpec::step` is the closed-form damped spring: starts at zero,
//! settles to exactly 1.0 within the documented tolerance, and never
//! overshoots when critically damped.

use fenestra_anim::SpringSpec;
use proptest::prelude::*;

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

/// An underdamped spring overshoots its target and the overshoot stays
/// within the decay envelope.
#[test]
fn underdamped_spring_overshoots_within_envelope() {
    let spring = SpringSpec {
        stiffness: 380.0,
        damping: 12.0,
    };
    let omega = spring.stiffness.sqrt();
    let zeta = spring.damping / (2.0 * omega);
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

/// The decay-rate half of `SpringSpec::step`'s own settle check
/// (`(-zeta.min(1.0) * omega * t).exp() < 0.001`), reproduced here so the
/// test can derive a settle-time bound analytically instead of guessing a
/// fixed wall-clock window that happens to work for "normal" springs. A
/// weak, lightly-damped spring (e.g. stiffness near 1, damping near 0.1)
/// genuinely takes much longer than a brisk UI spring to cross the 0.1%
/// envelope — that's correct physics, not a bug, so the bound must scale
/// with the spring's own parameters.
fn decay_rate(stiffness: f32, damping: f32) -> f32 {
    let omega = stiffness.max(1.0).sqrt();
    let zeta = damping.max(0.1) / (2.0 * omega);
    zeta.min(1.0) * omega
}

/// `t` at which `exp(-decay_rate * t)` first drops below 0.001, plus a 50%
/// margin so the test isn't sitting exactly on the boundary.
fn settle_bound(stiffness: f32, damping: f32) -> f32 {
    let rate = decay_rate(stiffness, damping);
    (6.908 / rate) * 1.5
}

proptest! {
    /// Any spring in a plausible parameter range (stiffness/damping both
    /// positive, a bounded launch velocity) settles to exactly 1.0 by its
    /// own analytically-derived settle bound, and stays settled after —
    /// the segment-end guarantee `Track::sample` relies on.
    #[test]
    fn any_spring_settles_to_the_segment_end_value(
        stiffness in 1.0f32..2000.0,
        damping in 0.1f32..200.0,
        v0 in -20.0f32..20.0,
    ) {
        let spring = SpringSpec { stiffness, damping };
        let t = settle_bound(stiffness, damping);
        let (x, settled) = spring.step(v0, t);
        prop_assert!(settled, "spring({stiffness}, {damping}) launched at v0={v0} has not settled by its own bound t={t}s");
        prop_assert_eq!(x, 1.0);
    }

    /// Once a spring reports settled at its own settle bound, a later time
    /// also reports settled at exactly 1.0 — settling isn't a one-frame
    /// fluke.
    #[test]
    fn settled_is_sticky(
        stiffness in 1.0f32..2000.0,
        damping in 0.1f32..200.0,
        v0 in -20.0f32..20.0,
    ) {
        let spring = SpringSpec { stiffness, damping };
        let t = settle_bound(stiffness, damping);
        let (_, settled) = spring.step(v0, t);
        prop_assert!(settled, "spring({stiffness}, {damping}) has not settled by its own bound t={t}s");
        let (x_later, settled_later) = spring.step(v0, t + 5.0);
        prop_assert!(settled_later);
        prop_assert_eq!(x_later, 1.0);
    }
}
