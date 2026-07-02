//! The data form: a versioned serde mirror of the composition IR. RON is
//! primary (with `implicit_some` + unwrapped variant newtypes for humane
//! authoring), JSON also parses; clip content embeds the fenestra-describe
//! node vocabulary; documents round-trip; and every failure is path-pointed.

use fenestra_motion::{Composition, Frames, Prop};

const LOWER_THIRD: &str = r#"(
    version: 1,
    width: 640,
    height: 360,
    fps: 30,
    duration: 90,
    background: "transparent",
    theme: dark,
    clips: [
        (
            id: "bar",
            start: 0,
            end: 90,
            z: -1,
            anchor: bottom_left,
            element: div(style: (bg: "accent", w: 320.0, h: 6.0)),
            tracks: [
                (prop: scale_xy, keys: [
                    (at: 0, value: pair(0.0, 1.0), ease: crisp),
                    (at: 20, value: pair(1.0, 1.0)),
                ]),
            ],
        ),
        (
            id: "title",
            start: 5,
            end: 90,
            anchor: bottom_left,
            element: text(content: "Q3 Revenue", style: (size_px: 40.0, weight: 600, color: "text")),
            tracks: [
                (prop: opacity, keys: [
                    (at: 0, value: scalar(0.0), ease: ease_out),
                    (at: 15, value: scalar(1.0)),
                ]),
                (prop: translate_y, keys: [
                    (at: 0, value: scalar(24.0), ease: crisp),
                    (at: 15, value: scalar(0.0)),
                ]),
                (prop: text_color, keys: [
                    (at: 0, value: color("text_muted")),
                    (at: 30, value: color("text"), ease: hold),
                ]),
            ],
        ),
    ],
    cuts: [45],
)"#;

#[test]
fn ron_document_compiles_and_samples() {
    let comp = Composition::from_ron(LOWER_THIRD).expect("parse + compile");
    assert_eq!(comp.width(), 640);
    assert_eq!(comp.fps(), 30);
    assert_eq!(comp.total_frames(), Frames(90));

    let scene = comp.sample(Frames(12));
    // z: -1 paints the bar under the title.
    assert_eq!(scene.paint_order(), ["bar", "title"]);

    let title = scene.resolve("title").unwrap();
    assert!(title.visible);
    // Clip-relative frame 7 of a 15-frame ease: opacity is mid-flight.
    assert!(title.props.opacity > 0.05 && title.props.opacity < 1.0);
    assert!(title.props.text_color.is_some(), "color track resolved");
    let bbox = title.bbox.expect("laid out");
    assert!(bbox.width() > 10.0);
}

#[test]
fn json_parses_the_same_document_shape() {
    let comp = Composition::from_ron(LOWER_THIRD).expect("ron");
    let json = comp.to_json().expect("doc-built comps serialize");
    let back = Composition::from_json(&json).expect("json parses");
    assert_eq!(back.width(), comp.width());
    assert_eq!(back.total_frames(), comp.total_frames());
    // Identical resolved values through either format.
    let a = comp.sample(Frames(12)).resolve("title").unwrap();
    let b = back.sample(Frames(12)).resolve("title").unwrap();
    assert_eq!(a.props, b.props);
}

#[test]
fn ron_round_trips() {
    let comp = Composition::from_ron(LOWER_THIRD).expect("ron");
    let out = comp.to_ron().expect("doc-built comps serialize");
    let again = Composition::from_ron(&out).expect("re-parse");
    let a = comp.sample(Frames(30)).resolve("bar").unwrap();
    let b = again.sample(Frames(30)).resolve("bar").unwrap();
    assert_eq!(a.props, b.props, "round-tripped doc resolves identically");
    // And the serialized doc is stable (fixpoint after one trip).
    assert_eq!(out, again.to_ron().expect("serialize again"));
}

#[test]
fn code_built_compositions_do_not_serialize() {
    let comp =
        Composition::new(64, 64, 30).clip(fenestra_motion::Clip::dynamic("d", 0..10, |_| {
            fenestra_core::div()
        }));
    let err = comp.to_ron().expect_err("closures cannot serialize");
    assert!(
        err.to_string().contains("data form"),
        "explains the data-form boundary: {err}"
    );
}

#[test]
fn mutating_a_loaded_composition_invalidates_serialization() {
    // A builder call after from_ron makes the composition code-built again
    // — to_ron must refuse rather than silently drop the mutation and
    // serialize the stale doc from disk.
    let comp = Composition::from_ron(LOWER_THIRD)
        .expect("ron")
        .cut(Frames(1));
    let err = comp
        .to_ron()
        .expect_err("a mutated composition is no longer the loaded document");
    assert!(
        err.to_string().contains("data form"),
        "explains the data-form boundary: {err}"
    );
}

#[test]
fn wrong_version_is_rejected() {
    let src = LOWER_THIRD.replace("version: 1", "version: 2");
    let err = Composition::from_ron(&src).expect_err("future versions are not guessed at");
    assert!(err.to_string().contains("version"), "{err}");
}

#[test]
fn track_value_kind_mismatch_is_path_pointed() {
    let src = LOWER_THIRD.replace("value: scalar(0.0)", "value: pair(0.0, 1.0)");
    let err = Composition::from_ron(&src).expect_err("opacity is scalar");
    let msg = err.to_string();
    assert!(
        msg.contains("title") && msg.contains("opacity"),
        "points at the offending clip and prop: {msg}"
    );
}

#[test]
fn duplicate_clip_ids_error_cleanly() {
    let src = LOWER_THIRD.replace(r#"id: "bar""#, r#"id: "title""#);
    let err = Composition::from_ron(&src).expect_err("ids must be unique");
    assert!(err.to_string().contains("title"), "{err}");
}

#[test]
fn unknown_element_vocabulary_is_reported() {
    let src = LOWER_THIRD.replace("size_px: 40.0", "font_size: 40.0");
    let err = Composition::from_ron(&src).expect_err("unknown style fields are rejected");
    assert!(
        err.to_string().contains("font_size") || err.to_string().contains("unknown"),
        "{err}"
    );
}

#[test]
fn shipped_lower_third_example_compiles_and_round_trips() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/lower_third.ron"
    ))
    .expect("shipped example exists");
    let comp = Composition::from_ron(&src).expect("the shipped example must always compile");
    assert_eq!(comp.fps(), 60);
    assert_eq!(comp.total_frames(), Frames(240));
    assert_eq!(comp.clip_ids(), ["plate", "bar", "title", "subtitle"]);
    // Mid-entrance: the title is rising and fading in.
    let title = comp.sample(Frames(18)).resolve("title").unwrap();
    assert!(title.visible);
    assert!(title.props.opacity > 0.0 && title.props.opacity < 1.0);
    assert!(title.props.translate.1 > 0.0);
    // And it round-trips.
    let again = Composition::from_ron(&comp.to_ron().expect("serialize")).expect("re-parse");
    assert_eq!(again.clip_ids(), comp.clip_ids());
}

#[test]
fn hostile_span_start_errors_instead_of_panicking() {
    // start = u64::MAX with end < start: the compile must report, not
    // overflow (`start + 1`) — every CLI subcommand routes through here.
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 18446744073709551615, end: 0, element: div(),
    )])"#;
    let err = Composition::from_ron(src).expect_err("hostile spans error cleanly");
    assert!(err.to_string().contains("span"), "{err}");
}

#[test]
fn zero_canvas_dimensions_are_rejected_not_clamped() {
    let src = r#"(version: 1, width: 0, height: 100, fps: 0, clips: [(
        id: "x", start: 0, end: 10, element: div(),
    )])"#;
    let err = Composition::from_ron(src).expect_err("zero dims are author errors");
    let msg = err.to_string();
    assert!(msg.contains("width"), "{msg}");
    assert!(msg.contains("fps"), "{msg}");
}

#[test]
fn non_finite_spring_parameters_are_rejected() {
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10, element: div(),
        tracks: [(prop: opacity, keys: [
            (at: 0, value: scalar(0.0), ease: spring(stiffness: inf, damping: 26.0)),
            (at: 5, value: scalar(1.0)),
        ])],
    )])"#;
    let err = Composition::from_ron(src).expect_err("inf stiffness is not a spring");
    assert!(err.to_string().contains("finite"), "{err}");
}

#[test]
fn non_finite_scalar_values_are_rejected() {
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10, element: div(),
        tracks: [(prop: opacity, keys: [
            (at: 0, value: scalar(inf)),
            (at: 5, value: scalar(1.0)),
        ])],
    )])"#;
    let err = Composition::from_ron(src).expect_err("inf is not a valid opacity");
    assert!(err.to_string().contains("finite"), "{err}");
}

#[test]
fn nan_scalar_value_is_rejected() {
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10, element: div(),
        tracks: [(prop: translate_x, keys: [
            (at: 0, value: scalar(NaN)),
            (at: 5, value: scalar(10.0)),
        ])],
    )])"#;
    let err = Composition::from_ron(src).expect_err("NaN must not silently pass every lint");
    assert!(err.to_string().contains("finite"), "{err}");
}

#[test]
fn non_finite_pair_value_is_rejected() {
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10, element: div(),
        tracks: [(prop: scale_xy, keys: [
            (at: 0, value: pair(inf, 1.0)),
            (at: 5, value: pair(1.0, 1.0)),
        ])],
    )])"#;
    let err = Composition::from_ron(src).expect_err("inf in a pair value must error");
    assert!(err.to_string().contains("finite"), "{err}");
}

#[test]
fn a_color_track_can_animate_to_transparent() {
    // "transparent" is accepted for the background field; a color TRACK
    // must accept the identical grammar — fading a fill to nothing is a
    // basic motion move, not just a canvas-level setting.
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10,
        element: div(style: (w: 10.0, h: 10.0, bg: "accent")),
        tracks: [(prop: fill_color, keys: [
            (at: 0, value: color("accent")),
            (at: 9, value: color("transparent")),
        ])],
    )])"#;
    let comp = Composition::from_ron(src).expect("transparent is a valid color-track value");
    let end = comp.sample(Frames(9)).resolve("x").unwrap().props.fill;
    assert_eq!(
        end,
        Some(fenestra_core::Color::TRANSPARENT),
        "the track settles on true transparency: {end:?}"
    );
}

#[test]
fn custom_anchor_and_srgb_space_round_trip() {
    let src = r#"(version: 1, width: 100, height: 100, fps: 30, clips: [(
        id: "x", start: 0, end: 10,
        anchor: custom(0.2, 0.8),
        element: div(style: (w: 10.0, h: 10.0)),
        tracks: [(prop: fill_color, space: srgb, keys: [
            (at: 0, value: color("accent")),
            (at: 5, value: color("danger")),
        ])],
    )])"#;
    let comp = Composition::from_ron(src).expect("compiles");
    let out = comp.to_ron().expect("serializes");
    assert!(out.contains("custom"), "{out}");
    assert!(out.contains("srgb"), "{out}");
    let again = Composition::from_ron(&out).expect("re-parses");
    let a = comp.sample(Frames(3)).resolve("x").unwrap().props;
    let b = again.sample(Frames(3)).resolve("x").unwrap().props;
    assert_eq!(a, b);
}

#[test]
fn hand_written_json_parses_the_documented_shape() {
    let src = r#"{
        "version": 1, "width": 100, "height": 100, "fps": 30,
        "clips": [{
            "id": "x", "start": 0, "end": 10,
            "element": {"text": {"content": "hi"}},
            "tracks": [{"prop": "opacity", "keys": [
                {"at": 0, "value": {"scalar": 0.0}, "ease": "crisp"},
                {"at": 5, "value": {"scalar": 1.0}}
            ]}]
        }]
    }"#;
    let comp = Composition::from_json(src).expect("hand-written JSON parses");
    let props = comp.sample(Frames(5)).resolve("x").unwrap().props;
    assert_eq!(props.opacity, 1.0);
}

#[test]
fn prop_names_serialize_snake_case() {
    // The doc grammar speaks snake_case; spot-check the enum mapping.
    let comp = Composition::from_ron(LOWER_THIRD).expect("ron");
    let ron = comp.to_ron().expect("serialize");
    assert!(ron.contains("translate_y"), "{ron}");
    assert!(!ron.contains("TranslateY"), "no Rust casing leaks: {ron}");
    let _ = Prop::TranslateY; // the prop this guards
}
