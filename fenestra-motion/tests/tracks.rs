//! Color track semantics: the extrapolation rule (overshoot curves clamp
//! colors, unlike numeric tracks — see `fenestra-anim`'s `tests/track.rs`
//! for the shared hold/exact-hit/easing semantics), and Oklab-default color
//! interpolation with per-track sRGB opt-in.

use fenestra_core::Color;
use fenestra_motion::{ColorTrack, EASE_POP, Frames, key};

#[test]
fn overshoot_clamps_colors_at_the_target() {
    let a = Color::new([0.0, 0.0, 0.0, 1.0]);
    let b = Color::new([0.5, 0.5, 0.5, 1.0]);
    let track = ColorTrack::new([key(0, a).ease(EASE_POP), key(30, b)]);
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
fn colors_lerp_oklab_by_default_with_srgb_opt_in() {
    // Blue to yellow: the perceptual (Oklab) midpoint differs visibly from
    // the raw sRGB component midpoint.
    let blue = Color::new([0.0, 0.0, 1.0, 1.0]);
    let yellow = Color::new([1.0, 1.0, 0.0, 1.0]);

    let oklab = ColorTrack::new([key(0, blue), key(10, yellow)]);
    let srgb = ColorTrack::new([key(0, blue), key(10, yellow)]).srgb();

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

fn track_mid(track: &ColorTrack) -> [f32; 4] {
    track.sample(Frames(5), 60).components
}

#[test]
#[should_panic(expected = "duplicate key frame")]
fn duplicate_key_frames_panic() {
    let _ = ColorTrack::new([
        key(5, Color::new([0.0, 0.0, 0.0, 1.0])),
        key(5, Color::new([1.0, 1.0, 1.0, 1.0])),
    ]);
}

#[test]
#[should_panic(expected = "at least one key")]
fn empty_track_panics() {
    let _ = ColorTrack::new([]);
}
