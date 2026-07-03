//! Track sampling semantics: hold at the ends, exact key hits, per-segment
//! easing (bezier / hold / closed-form spring), and the extrapolation rule
//! (overshoot curves extrapolate numeric values past their endpoint).

use fenestra_anim::{Frames, Track, ease_out, hold, key, spring};

#[test]
fn track_holds_before_first_and_after_last_and_hits_keys_exactly() {
    let track = Track::new([key(10, 2.0f32), key(20, 6.0)]);
    // Hold before the first key.
    assert_eq!(track.sample(Frames(0), 60), 2.0);
    assert_eq!(track.sample(Frames(9), 60), 2.0);
    // Exact hits return the key values.
    assert_eq!(track.sample(Frames(10), 60), 2.0);
    assert_eq!(track.sample(Frames(20), 60), 6.0);
    // Hold after the last key.
    assert_eq!(track.sample(Frames(21), 60), 6.0);
    assert_eq!(track.sample(Frames(1000), 60), 6.0);
}

#[test]
fn single_key_track_is_a_constant() {
    let track = Track::new([key(30, 5.0f32)]);
    for frame in [0u64, 29, 30, 31, 500] {
        assert_eq!(track.sample(Frames(frame), 60), 5.0);
    }
}

#[test]
fn linear_segment_interpolates() {
    let track = Track::new([key(0, 0.0f32), key(10, 10.0)]);
    assert_eq!(track.sample(Frames(5), 60), 5.0);
    assert_eq!(track.sample(Frames(2), 60), 2.0);
}

#[test]
fn bezier_ease_shapes_a_segment() {
    // ease_out leads the diagonal early: at 25% of the segment the value is
    // past 25% of the way.
    let track = Track::new([key(0, 0.0f32).ease(ease_out()), key(100, 1.0)]);
    let quarter = track.sample(Frames(25), 60);
    assert!(
        quarter > 0.25 + 0.05,
        "ease_out should lead linear at 25%: {quarter}"
    );
    assert_eq!(track.sample(Frames(100), 60), 1.0);
}

#[test]
fn hold_ease_holds_until_the_next_key() {
    let track = Track::new([key(0, 1.0f32).ease(hold()), key(10, 9.0)]);
    assert_eq!(track.sample(Frames(0), 60), 1.0);
    assert_eq!(track.sample(Frames(9), 60), 1.0);
    // The next key still hits exactly.
    assert_eq!(track.sample(Frames(10), 60), 9.0);
}

#[test]
fn overshoot_bezier_extrapolates_numeric_values_mid_segment() {
    // A y control point exceeding 1 (a "pop" preset): somewhere mid-segment
    // the eased progress passes 1.0, and a numeric track must extrapolate
    // past the end value, while the final key still lands exactly.
    let pop = fenestra_anim::Ease::Bezier(fenestra_anim::CubicBezier {
        x1: 0.34,
        y1: 1.56,
        x2: 0.64,
        y2: 1.0,
    });
    let track = Track::new([key(0, 0.0f32).ease(pop), key(30, 100.0)]);
    let peak = (0..=30)
        .map(|f| track.sample(Frames(f), 60))
        .fold(f32::MIN, f32::max);
    assert!(
        peak > 100.0 + 1.0,
        "overshoot curve should push a numeric value past its target: peak {peak}"
    );
    assert_eq!(track.sample(Frames(30), 60), 100.0);
}

#[test]
fn spring_segment_settles_to_the_end_value() {
    // A 2-second segment at 60fps: stiffness 380 / damping 26 settles well
    // inside it, so late frames sit on the target and the next key is exact.
    let track = Track::new([key(0, 0.0f32).ease(spring(380.0, 26.0)), key(120, 50.0)]);
    let early = track.sample(Frames(6), 60);
    assert!(
        early > 0.0 && early < 50.0,
        "spring is mid-flight early on: {early}"
    );
    let late = track.sample(Frames(115), 60);
    assert!(
        (late - 50.0).abs() < 0.05,
        "spring has settled near the target late in the segment: {late}"
    );
    assert_eq!(track.sample(Frames(120), 60), 50.0);
    assert_eq!(track.sample(Frames(121), 60), 50.0);
}

#[test]
fn underdamped_spring_overshoots_the_segment_target() {
    let track = Track::new([key(0, 0.0f32).ease(spring(380.0, 10.0)), key(120, 50.0)]);
    let peak = (0..=120)
        .map(|f| track.sample(Frames(f), 60))
        .fold(f32::MIN, f32::max);
    assert!(
        peak > 50.5,
        "an underdamped spring should overshoot its target: peak {peak}"
    );
}

#[test]
#[should_panic(expected = "duplicate key frame")]
fn duplicate_key_frames_panic() {
    let _ = Track::new([key(5, 1.0f32), key(5, 2.0)]);
}

#[test]
#[should_panic(expected = "at least one key")]
fn empty_track_panics() {
    let _ = Track::<f32>::new([]);
}

#[test]
fn keys_sort_by_frame_regardless_of_authoring_order() {
    let track = Track::new([key(20, 6.0f32), key(10, 2.0)]);
    assert_eq!(track.sample(Frames(0), 60), 2.0);
    assert_eq!(track.sample(Frames(15), 60), 4.0);
    assert_eq!(track.sample(Frames(25), 60), 6.0);
}

#[test]
fn pair_track_interpolates_componentwise() {
    let track = Track::new([key(0, (0.0f32, 10.0f32)), key(10, (10.0, 0.0))]);
    assert_eq!(track.sample(Frames(5), 60), (5.0, 5.0));
}
