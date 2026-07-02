//! The verification layer — the point of the crate: structural assertions
//! over sampled scenes (no pixels), temporal lints over frame ranges
//! (discontinuities, monotonicity, settling), auto-selected sentinel
//! frames, and the contact sheet.

use fenestra_core::div;
use fenestra_motion::verify::{Direction, discontinuities, monotone, settled};
use fenestra_motion::{Clip, Composition, Frames, Prop, Track, hold, key};

fn box_clip(id: &str, span: std::ops::Range<u64>) -> Clip {
    Clip::new(id, span).element(|| div().w(100.0).h(40.0))
}

#[test]
fn sentinel_frames_cover_keys_midpoints_and_span_edges() {
    let comp = Composition::new(320, 180, 30).duration(Frames(60)).clip(
        box_clip("a", 10..40).animate(Prop::Opacity, Track::new([key(0, 0.0f32), key(10, 1.0)])),
    );
    let sentinels = comp.sentinel_frames();
    // Span edges (10, 40), comp-absolute keys (10, 20), the segment
    // midpoint (15) — deduped and sorted.
    assert_eq!(
        sentinels,
        vec![Frames(10), Frames(15), Frames(20), Frames(40)]
    );
}

#[test]
fn sentinels_clamp_to_the_duration_and_dedup() {
    let comp = Composition::new(320, 180, 30).duration(Frames(30)).clip(
        box_clip("a", 0..60).animate(Prop::Opacity, Track::new([key(0, 0.0f32), key(50, 1.0)])),
    );
    let sentinels = comp.sentinel_frames();
    assert!(sentinels.iter().all(|f| f.0 < 30), "{sentinels:?}");
    let mut deduped = sentinels.clone();
    deduped.dedup();
    assert_eq!(sentinels, deduped, "no duplicates");
}

#[test]
fn an_undeclared_jump_is_a_discontinuity() {
    // A hold segment snaps at its next key: a mid-span jump.
    let comp =
        Composition::new(320, 180, 30)
            .duration(Frames(40))
            .clip(box_clip("j", 0..40).animate(
                Prop::TranslateX,
                Track::new([key(0, 0.0f32).ease(hold()), key(20, 150.0)]),
            ));
    let report = discontinuities(&comp, None);
    assert_eq!(report.len(), 1, "{report:?}");
    assert_eq!(report[0].clip, "j");
    assert_eq!(report[0].frame, Frames(20));

    // Declaring the cut blesses the same jump.
    let blessed = Composition::new(320, 180, 30)
        .duration(Frames(40))
        .cut(Frames(20))
        .clip(box_clip("j", 0..40).animate(
            Prop::TranslateX,
            Track::new([key(0, 0.0f32).ease(hold()), key(20, 150.0)]),
        ));
    assert!(discontinuities(&blessed, None).is_empty());
}

#[test]
fn span_edges_are_not_discontinuities() {
    // Appearing at frame 10 with opacity 1 is an entrance, not a glitch.
    let comp = Composition::new(320, 180, 30)
        .duration(Frames(30))
        .clip(box_clip("e", 10..25));
    assert!(discontinuities(&comp, None).is_empty());
}

#[test]
fn monotone_verifies_direction_over_a_range() {
    let comp = Composition::new(320, 180, 30).duration(Frames(30)).clip(
        box_clip("m", 0..30).animate(Prop::Opacity, Track::new([key(0, 0.0f32), key(20, 1.0)])),
    );
    assert!(
        monotone(&comp, "m", Prop::Opacity, 0..20, Direction::Increasing).is_empty(),
        "a plain ramp is monotone"
    );
    let wiggle =
        Composition::new(320, 180, 30)
            .duration(Frames(30))
            .clip(box_clip("w", 0..30).animate(
                Prop::Opacity,
                Track::new([key(0, 0.0f32), key(10, 1.0), key(20, 0.5)]),
            ));
    let problems = monotone(&wiggle, "w", Prop::Opacity, 0..20, Direction::Increasing);
    assert!(!problems.is_empty(), "the dip after frame 10 is caught");
}

#[test]
fn monotone_clamps_the_local_frame_to_the_span_like_the_renderer_does() {
    // Span 0..10 (span.len = 10, frozen local = 9). The track rises 0..9,
    // then a LATER segment (local 9..20) falls — but the clip is only
    // visible/animating up to local frame 9; every renderer (sample,
    // settled) freezes local at 9 for comp frames >= 10, so the value
    // holds flat at 100 rather than following the track down to 50.
    // monotone must read the same frozen value, not the raw track.
    let comp =
        Composition::new(320, 180, 30)
            .duration(Frames(20))
            .clip(box_clip("m", 0..10).animate(
                Prop::TranslateX,
                Track::new([key(0, 0.0f32), key(9, 100.0), key(20, 50.0)]),
            ));
    let problems = monotone(&comp, "m", Prop::TranslateX, 0..20, Direction::Increasing);
    assert!(
        problems.is_empty(),
        "the renderer freezes at 100 past span end; that's flat, not a decrease: {problems:?}"
    );
}

#[test]
fn settled_means_nothing_changes_afterward() {
    let comp =
        Composition::new(320, 180, 30)
            .duration(Frames(40))
            .clip(box_clip("s", 0..40).animate(
                Prop::TranslateY,
                Track::new([key(0, 24.0f32), key(15, 0.0)]),
            ));
    assert!(
        settled(&comp, Frames(15)).is_empty(),
        "still after frame 15"
    );
    let problems = settled(&comp, Frames(5));
    assert!(!problems.is_empty(), "still moving at frame 5");
    assert_eq!(problems[0].clip, "s");
}

#[test]
fn contact_sheet_tiles_every_nth_frame() {
    let comp = Composition::new(320, 180, 30)
        .duration(Frames(12))
        .background(fenestra_core::Color::new([0.1, 0.1, 0.12, 1.0]))
        .clip(box_clip("b", 0..12));
    let sheet = comp.contact_sheet(4, 160).expect("sheet");
    // Frames 0, 4, 8 at 160px-wide thumbs: the sheet fits them plus labels.
    assert!(
        sheet.width() >= 160,
        "at least one column: {}",
        sheet.width()
    );
    assert!(
        u64::from(sheet.width()) * u64::from(sheet.height()) > 160 * 90 * 3,
        "room for three labeled thumbnails"
    );
}

#[test]
fn contact_sheet_refuses_to_exceed_the_texture_ceiling() {
    let comp = Composition::new(1920, 1080, 60)
        .duration(Frames(100_000))
        .clip(box_clip("b", 0..100_000));
    let err = comp
        .contact_sheet(1, 1920)
        .expect_err("100k full-size thumbs cannot fit");
    assert!(
        err.to_string().contains("every"),
        "the error suggests the fix: {err}"
    );
}
