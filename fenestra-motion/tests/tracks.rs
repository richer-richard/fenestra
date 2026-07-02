//! Track sampling semantics: hold at the ends, exact key hits, per-segment
//! easing (bezier / hold / closed-form spring), the extrapolation rule
//! (overshoot curves extrapolate scalars, clamp colors), and Oklab-default
//! color interpolation with per-track sRGB opt-in.

use fenestra_core::Color;
use fenestra_motion::{EASE_POP, Frames, Track, ease_out, hold, key, spring};

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
fn overshoot_bezier_extrapolates_scalars_mid_segment() {
    // EASE_POP's y control point exceeds 1: somewhere mid-segment the eased
    // progress passes 1.0, and a scalar track must extrapolate past the end
    // value (that is the pop), while the final key still lands exactly.
    let track = Track::new([key(0, 0.0f32).ease(EASE_POP), key(30, 100.0)]);
    let peak = (0..=30)
        .map(|f| track.sample(Frames(f), 60))
        .fold(f32::MIN, f32::max);
    assert!(
        peak > 100.0 + 1.0,
        "overshoot curve should push a scalar past its target: peak {peak}"
    );
    assert_eq!(track.sample(Frames(30), 60), 100.0);
}

#[test]
fn overshoot_clamps_colors_at_the_target() {
    let a = Color::new([0.0, 0.0, 0.0, 1.0]);
    let b = Color::new([0.5, 0.5, 0.5, 1.0]);
    let track = Track::new([key(0, a).ease(EASE_POP), key(30, b)]);
    for f in 0..=30 {
        let c = track.sample(Frames(f), 60);
        for ch in 0..3 {
            assert!(
                c.components[ch] <= 0.5 + 1e-4,
                "colors clamp at the target under overshoot easing: frame {f} channel {ch} = {}",
                c.components[ch]
            );
        }
    }
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
fn colors_lerp_oklab_by_default_with_srgb_opt_in() {
    // Blue to yellow: the perceptual (Oklab) midpoint differs visibly from
    // the raw sRGB component midpoint.
    let blue = Color::new([0.0, 0.0, 1.0, 1.0]);
    let yellow = Color::new([1.0, 1.0, 0.0, 1.0]);

    let oklab = Track::new([key(0, blue), key(10, yellow)]);
    let srgb = Track::new([key(0, blue), key(10, yellow)]).srgb();

    // Endpoints are exact in both spaces.
    for track in [&oklab, &srgb] {
        assert_eq!(track.sample(Frames(0), 60).components, blue.components);
        assert_eq!(track.sample(Frames(10), 60).components, yellow.components);
    }

    let mid_oklab = track_mid(&oklab);
    let mid_srgb = track_mid(&srgb);
    // sRGB midpoint is the exact component average.
    for (ch, actual) in mid_srgb.iter().enumerate().take(3) {
        let expect = (blue.components[ch] + yellow.components[ch]) / 2.0;
        assert!(
            (actual - expect).abs() < 1e-3,
            "srgb midpoint channel {ch}: {actual} vs {expect}"
        );
    }
    // The Oklab midpoint takes a different path.
    let delta: f32 = (0..3).map(|ch| (mid_oklab[ch] - mid_srgb[ch]).abs()).sum();
    assert!(
        delta > 0.05,
        "Oklab and sRGB midpoints should differ perceptibly: delta {delta}"
    );
}

fn track_mid(track: &Track<Color>) -> [f32; 4] {
    track.sample(Frames(5), 60).components
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
