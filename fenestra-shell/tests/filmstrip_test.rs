//! `Harness::film` (deferred item #6, ARCHITECTURE.md) and
//! `assert_filmstrip_snapshot`: capturing a sequence of renders across the
//! deterministic clock so an agent can watch a transition play, and
//! composing that sequence into one reviewable strip.

use std::path::PathBuf;

use fenestra_core::{App, Element, Theme, Transition, col, div};
use fenestra_shell::testing::assert_filmstrip_snapshot;
use fenestra_shell::{Harness, MAX_FILM_FRAMES, MAX_FILM_INTERVAL_MS};
use image::RgbaImage;

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// Flips a panel between two theme colors through a 300ms crossfade —
/// deliberately the same shape as `fenestra-kit`'s motion test (`Mood`/
/// `Flip`), since it is the smallest real transition available without
/// adding a dependency just for this test.
struct Mood {
    alert: bool,
}

#[derive(Clone)]
struct Flip;

impl App for Mood {
    type Msg = Flip;

    fn update(&mut self, Flip: Flip) {
        self.alert = !self.alert;
    }

    fn view(&self) -> Element<Flip> {
        let alert = self.alert;
        col().p(10.0).children([div()
            .w(80.0)
            .h(80.0)
            .transition(Transition::colors().duration_ms(300.0))
            .themed(move |t: &Theme, s| {
                if alert {
                    s.bg(t.danger.solid)
                } else {
                    s.bg(t.accent)
                }
            })])
    }
}

/// A harness with real motion enabled and the panel's *initial* enter
/// state already settled, so the only transition left to observe is the
/// one under test ([`Flip`]).
fn mood_harness() -> Harness<Mood> {
    let mut h = Harness::new(Mood { alert: false }, Theme::light(), (200, 200));
    h.set_reduced_motion(false);
    h.pump(400.0);
    h
}

fn center(img: &RgbaImage) -> image::Rgba<u8> {
    *img.get_pixel(50, 50)
}

#[test]
fn film_captures_the_requested_frame_count_and_a_real_transition_differs() {
    let mut h = mood_harness();
    h.update(Flip);
    let frames = h.film(5, 100);
    assert_eq!(frames.len(), 5, "one frame per requested step");

    assert_ne!(
        center(&frames[0]),
        center(&frames[4]),
        "a 300ms crossfade over a 400ms span: the start and end colors differ"
    );
    assert_ne!(
        center(&frames[1]),
        center(&frames[0]),
        "the second frame (+100ms) has already moved off the starting color"
    );
}

#[test]
fn film_under_reduced_motion_is_static_by_default() {
    // The critical finding this feature turns on: headless rendering
    // defaults to reduced motion for single-shot golden stability, so
    // `film` without opting out of it is `frames` copies of one frame —
    // not because `film` suppresses anything itself, but because the
    // harness's default does. Determinism for real motion comes from the
    // clock, never from snapping animation, so the opt-out lives on
    // `set_reduced_motion`, not on `film`.
    let mut h = Harness::new(Mood { alert: false }, Theme::light(), (200, 200));
    h.update(Flip); // reduced motion is still the `Harness::new` default here
    let frames = h.film(4, 100);
    for (i, f) in frames.iter().enumerate().skip(1) {
        assert_eq!(
            center(f),
            center(&frames[0]),
            "frame {i}: reduced motion snaps instantly, so every frame matches the first"
        );
    }
}

/// The pixel-agreement bound the GPU actually provides in-process (see
/// ARCHITECTURE.md's determinism-contract section, and
/// `fenestra-motion/tests/render.rs`'s identical helper): the same scene
/// rendered twice can wobble +/-1 LSB on a sliver of antialiased pixels, so
/// two `film` runs are compared within that bound rather than requiring
/// byte-identity.
fn assert_pixels_close(a: &RgbaImage, b: &RgbaImage, what: &str) {
    assert_eq!(a.dimensions(), b.dimensions(), "{what}: dimensions");
    let mut differing = 0usize;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        let max = pa.0.iter().zip(pb.0).map(|(x, y)| x.abs_diff(y)).max();
        if let Some(d) = max
            && d > 0
        {
            assert!(
                d <= 1,
                "{what}: channel delta {d} exceeds the +/-1 LSB bound"
            );
            differing += 1;
        }
    }
    let total = (a.width() * a.height()) as usize;
    assert!(
        differing * 1000 <= total,
        "{what}: {differing}/{total} pixels differ (> 0.1%)"
    );
}

#[test]
fn film_is_deterministic_within_the_gpu_bound() {
    let run = || {
        let mut h = mood_harness();
        h.update(Flip);
        h.film(4, 75)
    };
    let a = run();
    let b = run();
    assert_eq!(a.len(), b.len());
    for (i, (fa, fb)) in a.iter().zip(&b).enumerate() {
        assert_pixels_close(fa, fb, &format!("frame {i}"));
    }
}

#[test]
fn film_zero_frames_floors_to_one() {
    let mut h = mood_harness();
    assert_eq!(
        h.film(0, 10).len(),
        1,
        "a zero-frame request is meaningless; floor to 1"
    );
}

#[test]
fn film_clamps_an_enormous_frame_count() {
    let mut h = mood_harness();
    assert_eq!(
        h.film(usize::MAX, 1).len(),
        MAX_FILM_FRAMES,
        "usize::MAX frames clamps to the documented ceiling instead of \
         attempting to render and allocate that many"
    );
}

#[test]
fn film_clamps_an_enormous_interval_to_the_documented_ceiling() {
    // u64::MAX ms cast to f64 and added to the clock would still be a
    // finite (if absurd) number of seconds, so this doesn't just check
    // "doesn't panic" — it proves the *value* used is the clamp, by
    // showing an u64::MAX-ms request lands exactly where an explicit
    // MAX_FILM_INTERVAL_MS request does.
    let mut clamped_run = mood_harness();
    clamped_run.update(Flip);
    let via_hostile_interval = clamped_run.film(2, u64::MAX);

    let mut ceiling_run = mood_harness();
    ceiling_run.update(Flip);
    let via_explicit_ceiling = ceiling_run.film(2, MAX_FILM_INTERVAL_MS);

    assert_pixels_close(
        &via_hostile_interval[1],
        &via_explicit_ceiling[1],
        "u64::MAX interval clamps to MAX_FILM_INTERVAL_MS",
    );
}

#[test]
fn filmstrip_golden() {
    let mut h = mood_harness();
    h.update(Flip);
    let frames = h.film(4, 150);
    assert_filmstrip_snapshot(snapshot_dir(), "mood_filmstrip", &frames, 150);
}

#[test]
fn filmstrip_snapshot_rejects_an_empty_filmstrip() {
    let frames: Vec<RgbaImage> = Vec::new();
    let result = std::panic::catch_unwind(|| {
        assert_filmstrip_snapshot(snapshot_dir(), "never_written", &frames, 100);
    });
    assert!(
        result.is_err(),
        "an empty filmstrip is a misuse, not a valid golden"
    );
}
