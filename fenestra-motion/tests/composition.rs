//! Composition and clip semantics: spans gate visibility, keyframes are
//! clip-relative, props resolve with identity defaults, ids and prop tracks
//! are unique, z-order sorts paint order, and bboxes reflect layout plus
//! paint-time transforms — all windowless (no GPU).

use fenestra_core::{Color, div, text};
use fenestra_motion::{Anchor, Clip, Composition, Frames, Prop, Track, key};

fn box_clip(id: &str, span: std::ops::Range<u64>) -> Clip {
    Clip::new(id, span).element(|| div().w(100.0).h(40.0))
}

#[test]
fn total_frames_is_bounded_regardless_of_declared_duration() {
    // A hostile-scale duration must not make sinks/CLI collect a
    // near-u64::MAX Vec<u64> or iterate forever.
    let comp = Composition::new(640, 360, 60).duration(Frames(u64::MAX));
    assert!(
        comp.total_frames().0 <= 10_000_000,
        "total_frames must clamp: {}",
        comp.total_frames()
    );
}

#[test]
fn total_frames_is_bounded_by_an_extreme_clip_span_too() {
    // No explicit duration: total_frames falls back to the furthest clip
    // end, which must be bounded the same way.
    let comp = Composition::new(640, 360, 60).clip(box_clip("huge", 0..u64::MAX));
    assert!(
        comp.total_frames().0 <= 10_000_000,
        "total_frames must clamp: {}",
        comp.total_frames()
    );
}

#[test]
fn clip_is_visible_only_within_its_span() {
    let comp = Composition::new(640, 360, 60).clip(box_clip("title", 10..20));
    assert!(!comp.sample(Frames(9)).resolve("title").unwrap().visible);
    assert!(comp.sample(Frames(10)).resolve("title").unwrap().visible);
    assert!(comp.sample(Frames(19)).resolve("title").unwrap().visible);
    assert!(!comp.sample(Frames(20)).resolve("title").unwrap().visible);
}

#[test]
fn resolve_of_an_unknown_id_is_none() {
    let comp = Composition::new(640, 360, 60).clip(box_clip("a", 0..10));
    assert!(comp.sample(Frames(0)).resolve("nope").is_none());
}

#[test]
fn keyframes_are_clip_relative() {
    // The clip starts at comp frame 40; its keys speak clip time.
    let comp = Composition::new(640, 360, 60).clip(box_clip("c", 40..100).animate(
        Prop::TranslateX,
        Track::new([key(0, 0.0f32), key(10, 10.0)]),
    ));
    let props = comp.sample(Frames(45)).resolve("c").unwrap().props;
    assert!((props.translate.0 - 5.0).abs() < 1e-6);
}

#[test]
fn props_default_to_identity() {
    let comp = Composition::new(640, 360, 60).clip(box_clip("c", 0..10));
    let props = comp.sample(Frames(5)).resolve("c").unwrap().props;
    assert_eq!(props.opacity, 1.0);
    assert_eq!(props.translate, (0.0, 0.0));
    assert_eq!(props.scale, 1.0);
    assert_eq!(props.scale_xy, (1.0, 1.0));
    assert_eq!(props.rotate, 0.0);
    assert!(props.fill.is_none());
    assert!(props.stroke.is_none());
    assert!(props.text_color.is_none());
}

#[test]
fn outside_the_span_props_clamp_and_the_clip_hides() {
    let comp = Composition::new(640, 360, 60).clip(
        box_clip("c", 10..20).animate(Prop::Opacity, Track::new([key(0, 0.2f32), key(5, 1.0)])),
    );
    let early = comp.sample(Frames(0)).resolve("c").unwrap();
    assert!(!early.visible);
    assert!(early.bbox.is_none());
    // Props sample at the clamped local frame (0): the first key's value.
    assert_eq!(early.props.opacity, 0.2);
}

#[test]
#[should_panic(expected = "duplicate clip id")]
fn duplicate_clip_ids_panic() {
    let _ = Composition::new(640, 360, 60)
        .clip(box_clip("x", 0..10))
        .clip(box_clip("x", 5..15));
}

#[test]
#[should_panic(expected = "already animated")]
fn duplicate_prop_tracks_panic() {
    let _ = box_clip("c", 0..10)
        .animate(Prop::Opacity, Track::new([key(0, 0.0f32)]))
        .animate(Prop::Opacity, Track::new([key(0, 1.0f32)]));
}

#[test]
#[should_panic(expected = "expects a")]
fn track_kind_must_match_the_prop() {
    let _ = box_clip("c", 0..10).animate(
        Prop::Opacity,
        Track::new([key(0, Color::new([1.0, 0.0, 0.0, 1.0]))]),
    );
}

#[test]
fn paint_order_is_insertion_order_with_z_overrides() {
    let comp = Composition::new(640, 360, 60)
        .clip(box_clip("top", 0..10).z(1))
        .clip(box_clip("under", 0..10))
        .clip(box_clip("base", 0..10).z(-1));
    let order = comp.sample(Frames(0)).paint_order();
    assert_eq!(order, ["base", "under", "top"]);
}

#[test]
fn bbox_centers_by_default_and_follows_translate_tracks() {
    let comp = Composition::new(600, 400, 60).clip(box_clip("card", 0..100).animate(
        Prop::TranslateX,
        Track::new([key(0, 0.0f32), key(10, 50.0)]),
    ));

    // Frame 0: a 100×40 box anchored center in a 600×400 canvas.
    let bbox = comp
        .sample(Frames(0))
        .resolve("card")
        .unwrap()
        .bbox
        .unwrap();
    assert!((bbox.x0 - 250.0).abs() < 1.0, "x0 {}", bbox.x0);
    assert!((bbox.y0 - 180.0).abs() < 1.0, "y0 {}", bbox.y0);
    assert!((bbox.width() - 100.0).abs() < 1.0);
    assert!((bbox.height() - 40.0).abs() < 1.0);

    // Frame 10: the translate track shifts the painted box right by 50.
    let moved = comp
        .sample(Frames(10))
        .resolve("card")
        .unwrap()
        .bbox
        .unwrap();
    assert!((moved.x0 - 300.0).abs() < 1.0, "moved x0 {}", moved.x0);
}

#[test]
fn anchor_places_the_clip_and_pivots_its_transforms() {
    // Top-left placement: the box sits at the canvas origin.
    let comp = Composition::new(600, 400, 60)
        .clip(box_clip("tl", 0..10).anchor(Anchor::TopLeft))
        .clip(
            box_clip("rot", 0..10)
                .anchor(Anchor::TopLeft)
                .animate(Prop::Rotate, Track::new([key(0, 90.0f32)])),
        );
    let scene = comp.sample(Frames(0));

    let tl = scene.resolve("tl").unwrap().bbox.unwrap();
    assert!(tl.x0.abs() < 1.0 && tl.y0.abs() < 1.0, "top-left at origin");

    // Rotating 90° about the top-left corner maps the 100×40 rect to an
    // AABB of (-40, 0)..(0, 100) around that corner.
    let rot = scene.resolve("rot").unwrap().bbox.unwrap();
    assert!((rot.x0 - -40.0).abs() < 1.0, "rot x0 {}", rot.x0);
    assert!((rot.x1 - 0.0).abs() < 1.0, "rot x1 {}", rot.x1);
    assert!((rot.y1 - 100.0).abs() < 1.0, "rot y1 {}", rot.y1);
}

#[test]
fn text_clips_measure_a_real_bbox() {
    let comp =
        Composition::new(600, 400, 60).clip(Clip::new("word", 0..10).element(|| text("Hello")));
    let bbox = comp
        .sample(Frames(0))
        .resolve("word")
        .unwrap()
        .bbox
        .unwrap();
    assert!(bbox.width() > 10.0, "text has real width: {}", bbox.width());
    assert!(bbox.height() > 5.0, "and height: {}", bbox.height());
}

#[test]
fn dynamic_clips_receive_the_clip_relative_frame() {
    let comp = Composition::new(600, 400, 60).clip(Clip::dynamic("bar", 20..80, |frame| {
        // Width grows one px per local frame: measurable via bbox.
        #[expect(clippy::cast_precision_loss, reason = "tiny test values")]
        div().w(10.0 + frame.0 as f32).h(10.0)
    }));
    let at_start = comp
        .sample(Frames(20))
        .resolve("bar")
        .unwrap()
        .bbox
        .unwrap();
    assert!((at_start.width() - 10.0).abs() < 1.0);
    let later = comp
        .sample(Frames(50))
        .resolve("bar")
        .unwrap()
        .bbox
        .unwrap();
    assert!((later.width() - 40.0).abs() < 1.0);
}
