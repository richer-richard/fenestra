//! The `serde` feature is opt-in and unexercised by the rest of the test
//! suite (which runs without it) — this file proves it actually round-trips
//! rather than just compiling. Only builds when the feature is enabled:
//! `cargo test -p fenestra-anim --features serde --test serde_feature`.

#![cfg(feature = "serde")]

use fenestra_anim::{CubicBezier, Ease, Frames, Key, Rounding, SpringSpec, Track, key};

#[test]
fn cubic_bezier_round_trips_through_json() {
    let curve = CubicBezier {
        x1: 0.2,
        y1: 0.0,
        x2: 0.0,
        y2: 1.0,
    };
    let json = serde_json::to_string(&curve).unwrap();
    let back: CubicBezier = serde_json::from_str(&json).unwrap();
    assert_eq!(curve, back);
}

#[test]
fn spring_spec_round_trips_through_json() {
    let spring = SpringSpec {
        stiffness: 380.0,
        damping: 26.0,
    };
    let json = serde_json::to_string(&spring).unwrap();
    let back: SpringSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spring, back);
}

#[test]
fn rounding_round_trips_through_json() {
    for mode in [Rounding::Floor, Rounding::Ceil, Rounding::Round] {
        let json = serde_json::to_string(&mode).unwrap();
        let back: Rounding = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }
}

#[test]
fn a_track_round_trips_through_json_and_samples_the_same() {
    let track: Track<f32> = Track::new([key(0, 0.0f32), key(10, 10.0)]);
    let json = serde_json::to_string(&track).unwrap();
    let back: Track<f32> = serde_json::from_str(&json).unwrap();
    for frame in [0u64, 3, 5, 10, 20] {
        assert_eq!(
            track.sample(Frames(frame), 60),
            back.sample(Frames(frame), 60)
        );
    }
}

#[test]
fn a_key_with_bezier_ease_round_trips() {
    let k: Key<f32> = key(0, 1.0).ease(Ease::Bezier(CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    }));
    let json = serde_json::to_string(&k).unwrap();
    let _back: Key<f32> = serde_json::from_str(&json).unwrap();
}
