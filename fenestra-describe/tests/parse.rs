//! Unit tests for the `Description` format, parser, and validation.

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame, oklch};
use fenestra_describe::color::{COLOR_ROLES, resolve_color};
use fenestra_describe::format::{ColorSpec, Description, OklchColor};
use fenestra_describe::parse::{to_element, to_element_lenient, validate};
use fenestra_describe::state::Action;

/// Builds a frame from an element and returns its aria snapshot (no GPU).
fn light_yaml(el: &Element<Action>) -> String {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        el,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (480.0, 320.0),
        1.0,
    );
    frame.access_yaml()
}

#[test]
fn parses_minimal_description() {
    let json = r#"{ "schema": "fenestra/1", "root": { "col": { "children": [
        { "text": { "content": "Hello" } }
    ] } } }"#;
    let desc: Description = serde_json::from_str(json).expect("valid description");
    assert_eq!(desc.schema, "fenestra/1");
}

#[test]
fn rejects_unknown_field_at_author_time() {
    let json = r#"{ "schema": "fenestra/1", "root": { "col": { "kids": [] } } }"#;
    let err = serde_json::from_str::<Description>(json).unwrap_err();
    assert!(err.to_string().contains("unknown field"), "got: {err}");
}

#[test]
fn resolves_role_color() {
    let t = Theme::light();
    assert_eq!(
        resolve_color(&ColorSpec::Role("surface".into()), &t).unwrap(),
        t.surface
    );
    assert_eq!(
        resolve_color(&ColorSpec::Role("accent".into()), &t).unwrap(),
        t.accent
    );
    assert_eq!(
        resolve_color(&ColorSpec::Role("danger".into()), &t).unwrap(),
        t.danger.solid
    );
}

#[test]
fn every_advertised_role_resolves() {
    let t = Theme::light();
    for role in COLOR_ROLES {
        assert!(
            resolve_color(&ColorSpec::Role((*role).into()), &t).is_ok(),
            "advertised role {role:?} failed to resolve"
        );
    }
}

#[test]
fn unknown_role_lists_valid_roles() {
    let t = Theme::light();
    let err = resolve_color(&ColorSpec::Role("primary".into()), &t).unwrap_err();
    assert!(
        err.message.contains("unknown color role"),
        "{}",
        err.message
    );
    assert!(
        err.message.contains("surface"),
        "error should list valid roles: {}",
        err.message
    );
}

#[test]
fn oklch_escape_hatch() {
    let t = Theme::light();
    let spec = ColorSpec::Oklch(OklchColor {
        oklch: [0.7, 0.1, 250.0],
    });
    assert_eq!(resolve_color(&spec, &t).unwrap(), oklch(0.7, 0.1, 250.0));
}

#[test]
fn parses_col_with_text_children() {
    let json = r#"{ "schema": "fenestra/1", "root": { "col": {
        "style": { "p": 16, "gap": 8 },
        "children": [
            { "text": { "content": "First" } },
            { "text": { "content": "Second" } }
        ]
    } } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).expect("parses");
    let yaml = light_yaml(&el);
    assert!(yaml.contains("First"), "{yaml}");
    assert!(yaml.contains("Second"), "{yaml}");
}

#[test]
fn bad_schema_is_an_error() {
    let json = r#"{ "schema": "fenestra/2", "root": { "col": {} } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let errs = to_element(&desc, &Theme::light()).err().unwrap();
    assert_eq!(errs[0].path, "schema");
}

#[test]
fn unknown_color_degrades_but_records_error() {
    let json = r#"{ "schema": "fenestra/1", "root": { "col": {
        "style": { "bg": "chartreuse" },
        "children": [ { "text": { "content": "Hi" } } ]
    } } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    // Lenient: the node still realizes (the text renders) and the error is recorded.
    let (el, errs) = to_element_lenient(&desc, &Theme::light());
    assert!(light_yaml(&el).contains("Hi"));
    assert_eq!(errs.len(), 1);
    assert_eq!(errs[0].path, "root/style/bg");
    // Strict: the same input is an error.
    assert!(to_element(&desc, &Theme::light()).is_err());
}

#[test]
fn button_has_accessible_name() {
    let json = r#"{ "schema": "fenestra/1", "root": { "button": { "label": "Add", "on_click": "add" } } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).unwrap();
    let yaml = light_yaml(&el);
    assert!(yaml.contains("button"), "{yaml}");
    assert!(yaml.contains("Add"), "{yaml}");
}

#[test]
fn checkbox_checked_shows_in_aria() {
    let json = r#"{ "schema": "fenestra/1", "root": { "checkbox": { "checked": true, "label": "Accept" } } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).unwrap();
    let yaml = light_yaml(&el);
    assert!(yaml.contains("checkbox"), "{yaml}");
    assert!(yaml.contains("[checked]"), "{yaml}");
}

#[test]
fn text_input_exposes_value() {
    let json = r#"{ "schema": "fenestra/1", "root": { "text_input": { "value": "draft", "on_input": "changed" } } }"#;
    let desc: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&desc, &Theme::light()).unwrap();
    let yaml = light_yaml(&el);
    assert!(yaml.contains("textbox"), "{yaml}");
    assert!(yaml.contains("draft"), "{yaml}");
}

#[test]
fn validate_accepts_valid() {
    assert!(validate(r#"{"schema":"fenestra/1","root":{"col":{"children":[]}}}"#).is_ok());
}

#[test]
fn validate_rejects_unknown_field_with_path() {
    let errs = validate(r#"{"schema":"fenestra/1","root":{"col":{"kids":[]}}}"#)
        .err()
        .unwrap();
    assert!(errs[0].message.contains("unknown field"), "{:?}", errs[0]);
    assert!(
        errs[0].path.contains("col") || errs[0].path.contains("root"),
        "path should locate the node: {}",
        errs[0].path
    );
}

#[test]
fn validate_rejects_bad_variant_tag() {
    let errs = validate(r#"{"schema":"fenestra/1","root":{"frobnicate":{}}}"#)
        .err()
        .unwrap();
    assert!(errs[0].message.contains("unknown variant"), "{:?}", errs[0]);
}

#[test]
fn validate_catches_semantic_color_error() {
    // Structurally valid JSON, but `taupe` is not a theme role.
    let errs = validate(r#"{"schema":"fenestra/1","root":{"col":{"style":{"bg":"taupe"}}}}"#)
        .err()
        .unwrap();
    assert!(
        errs.iter().any(|e| e.path.contains("bg")),
        "expected a bg-path error: {errs:?}"
    );
}

#[test]
fn button_variant_and_slider_step() {
    let t = Theme::light();
    // A valid variant builds.
    let d: Description = serde_json::from_str(
        r#"{"schema":"fenestra/1","root":{"button":{"label":"Delete","variant":"danger","on_click":"del"}}}"#,
    )
    .unwrap();
    assert!(to_element(&d, &t).is_ok());
    // An unknown variant degrades (the button still realizes) and records a path error.
    let d2: Description = serde_json::from_str(
        r#"{"schema":"fenestra/1","root":{"button":{"label":"X","variant":"neon"}}}"#,
    )
    .unwrap();
    let (el, errs) = to_element_lenient(&d2, &t);
    assert!(light_yaml(&el).contains("X"));
    assert!(
        errs.iter().any(|e| e.path.ends_with("/variant")),
        "{errs:?}"
    );
    // Slider step is accepted.
    let d3: Description = serde_json::from_str(
        r#"{"schema":"fenestra/1","root":{"slider":{"value":0.5,"step":0.25}}}"#,
    )
    .unwrap();
    assert!(to_element(&d3, &t).is_ok());
}

#[test]
fn non_finite_size_px_is_rejected_and_render_is_safe() {
    let t = Theme::light();
    // 1e300 parses as f64 then overflows f32 to +inf; a non-finite font size
    // panics parley's line breaker. validate() must reject it up front, and a
    // lenient parse must degrade so building a frame never panics.
    let json =
        r#"{"schema":"fenestra/1","root":{"text":{"content":"x","style":{"size_px":1e300}}}}"#;
    let d: Description = serde_json::from_str(json).unwrap();
    let (el, errs) = to_element_lenient(&d, &t);
    // Degraded: the bad size is dropped and an error is recorded.
    assert!(
        errs.iter().any(|e| e.path.contains("size_px")),
        "lenient parse should record the bad size_px: {errs:?}"
    );
    let _ = light_yaml(&el); // must not panic on a non-finite font size
    // Strict validate rejects it with a path-pointed error.
    let verrs = validate(json).expect_err("non-finite size_px must be rejected");
    assert!(
        verrs.iter().any(|e| e.path.contains("size_px")),
        "{verrs:?}"
    );
}

#[test]
fn out_of_range_oklch_is_rejected() {
    // Lightness -5 is outside the 0..=1 OKLCH range; the escape hatch must not
    // bless a degenerate (possibly NaN) color that validate() calls valid.
    let json =
        r#"{"schema":"fenestra/1","root":{"div":{"style":{"bg":{"oklch":[-5.0,0.0,0.0]}}}}}"#;
    let verrs = validate(json).expect_err("out-of-range oklch must be rejected");
    assert!(verrs.iter().any(|e| e.path.contains("bg")), "{verrs:?}");
}

#[test]
fn bound_widget_renders_from_initial_state() {
    let t = Theme::light();
    // A bound checkbox reads its initial checked state from the `state` map.
    let d: Description = serde_json::from_str(
        r#"{"schema":"fenestra/1","state":{"agreed":true},"root":{"checkbox":{"bind":"agreed","label":"Agree"}}}"#,
    )
    .unwrap();
    let el = to_element(&d, &t).unwrap();
    assert!(
        light_yaml(&el).contains("[checked]"),
        "a bound checkbox should render its initial state"
    );
    // A bound input shows its initial state value.
    let d2: Description = serde_json::from_str(
        r#"{"schema":"fenestra/1","state":{"name":"Ada"},"root":{"text_input":{"bind":"name"}}}"#,
    )
    .unwrap();
    assert!(light_yaml(&to_element(&d2, &t).unwrap()).contains("Ada"));
}

// ── Glass / material authoring (the moat: author the signature visual in JSON,
//    then verify it headlessly) ────────────────────────────────────────────────

#[test]
fn parses_explicit_glass_optics_into_style() {
    // The optics fields set the element's style directly (not deferred like a
    // surface role), so they read back immediately from `style()`.
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{
        "corner_smoothing":0.6,
        "backdrop_blur":24,
        "specular_edge":"glass",
        "sheen":{"light_deg":135,"top":0.12,"bottom":0.06},
        "adaptive_tint":{"pivot":0.55,"gain":0.2}
    },"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).expect("valid description");
    let el = to_element(&d, &Theme::light()).expect("parses");
    let s = el.style();
    assert_eq!(s.corner_smoothing, Some(0.6));
    assert!(s.backdrop_blur.is_some(), "backdrop_blur set");
    assert!(
        s.specular_edge.is_some(),
        "specular rim from the \"glass\" preset"
    );
    let sheen = s.sheen.expect("sheen set");
    assert!((sheen.top - 0.12).abs() < 1e-6 && (sheen.bottom - 0.06).abs() < 1e-6);
    let adaptive = s.adaptive_tint.expect("adaptive tint set");
    assert!((adaptive.pivot - 0.55).abs() < 1e-6 && (adaptive.gain - 0.2).abs() < 1e-6);
}

#[test]
fn glass_presets_match_the_core_recipe() {
    // The `"glass"` preset strings resolve to the exact recipes Surface::Glass uses.
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{
        "specular_edge":"glass","sheen":"glass","adaptive_tint":"glass"
    },"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&d, &Theme::light()).unwrap();
    let s = el.style();
    assert_eq!(s.specular_edge, Some(fenestra_core::SpecularEdge::glass()));
    assert_eq!(s.sheen, Some(fenestra_core::Sheen::glass()));
    assert_eq!(s.adaptive_tint, Some(fenestra_core::AdaptiveTint::glass()));
}

#[test]
fn surface_glass_role_builds_a_frame() {
    // `surface` is a deferred role (it resolves against the theme at frame time),
    // so the proof is that it parses and builds a frame with its content intact.
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{"surface":"glass"},"children":[
        {"text":{"content":"Frosted"}}
    ]}}}"#;
    let d: Description = serde_json::from_str(json).unwrap();
    let el = to_element(&d, &Theme::light()).expect("glass surface parses");
    assert!(light_yaml(&el).contains("Frosted"));
}

#[test]
fn rejects_unknown_surface_role() {
    let json =
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"surface":"frosted"},"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).unwrap();
    let errs = to_element(&d, &Theme::light())
        .err()
        .expect("unknown role errors");
    assert!(
        errs.iter().any(|e| e.message.contains("surface role")),
        "{errs:?}"
    );
}

#[test]
fn rejects_unknown_glass_preset() {
    let json =
        r#"{"schema":"fenestra/1","root":{"col":{"style":{"sheen":"frost"},"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).unwrap();
    let errs = to_element(&d, &Theme::light())
        .err()
        .expect("unknown preset errors");
    assert!(
        errs.iter().any(|e| e.message.contains("sheen preset")),
        "{errs:?}"
    );
}

#[test]
fn parses_transforms_corners_and_filter() {
    // Transforms + per-corner radii on one node...
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{
        "corners":[4,8,12,16],
        "translate":[10,5],"rotate":15,"skew":[2,0]
    },"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).expect("valid description");
    let el = to_element(&d, &Theme::light()).expect("parses");
    let s = el.style();
    assert_eq!((s.corner_radius.tl, s.corner_radius.bl), (4.0, 16.0));
    assert_eq!(s.translate, (10.0, 5.0));
    assert_eq!(s.rotate, 15.0);
    assert_eq!(s.skew, (2.0, 0.0));
    // ...and a foreground filter on a node WITHOUT a transform (they don't compose).
    let fjson = r#"{"schema":"fenestra/1","root":{"div":{"style":{"element_filter":{"brightness":0.5}},"children":[]}}}"#;
    let fd: Description = serde_json::from_str(fjson).unwrap();
    let fel = to_element(&fd, &Theme::light()).expect("parses");
    assert!(
        matches!(fel.style().element_filter, Some(fenestra_core::ElementFilter::Brightness(m)) if (m - 0.5).abs() < 1e-6),
        "{:?}",
        fel.style().element_filter
    );
}

#[test]
fn clamps_huge_authored_blur() {
    // A hostile blur radius is capped, not passed through to overflow the blur.
    let json = r#"{"schema":"fenestra/1","root":{"div":{"style":{"backdrop_blur":1000000000},"children":[]}}}"#;
    let el = to_element(
        &serde_json::from_str::<Description>(json).unwrap(),
        &Theme::light(),
    )
    .expect("parses");
    assert!(
        el.style().backdrop_blur.unwrap() <= 200.0,
        "{:?}",
        el.style().backdrop_blur
    );
    // material.blur is capped the same way (and still sets a translucent fill).
    let mjson = r#"{"schema":"fenestra/1","root":{"div":{"style":{"material":{"tint":"surface","fill_alpha":0.5,"blur":1000000000,"saturation":1.5}},"children":[]}}}"#;
    let mel = to_element(
        &serde_json::from_str::<Description>(mjson).unwrap(),
        &Theme::light(),
    )
    .expect("parses");
    assert!(mel.style().backdrop_blur.unwrap() <= 200.0);
    assert!(mel.style().fill.is_some());
}

#[test]
fn rejects_transform_with_element_filter() {
    // The two-pass filter crop is taken pre-transform, so the pair is rejected.
    let json = r#"{"schema":"fenestra/1","root":{"div":{"style":{"translate":[10,0],"element_filter":{"blur":4}},"children":[]}}}"#;
    let errs = to_element(
        &serde_json::from_str::<Description>(json).unwrap(),
        &Theme::light(),
    )
    .err()
    .expect("the combination is rejected");
    assert!(
        errs.iter().any(|e| e.message.contains("does not compose")),
        "{errs:?}"
    );
}

#[test]
fn parses_material_vibrancy_background() {
    // A custom glassier-than-stock material: a translucent fill + its backdrop blur,
    // authored in one field (the escape hatch behind `surface: "glass"`).
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{
        "material":{"tint":{"oklch":[0.7,0.05,250]},"fill_alpha":0.5,"blur":24,"saturation":1.6}
    },"children":[]}}}"#;
    let d: Description = serde_json::from_str(json).expect("valid description");
    let el = to_element(&d, &Theme::light()).expect("parses");
    let s = el.style();
    assert!(s.fill.is_some(), "material sets a translucent fill");
    assert_eq!(
        s.backdrop_blur,
        Some(24.0),
        "material drives the backdrop blur"
    );
}

#[test]
fn rejects_unknown_material_field() {
    // `deny_unknown_fields` on MaterialSpec catches a typo'd lever at deserialize.
    let json = r#"{"schema":"fenestra/1","root":{"col":{"style":{
        "material":{"tint":"accent","fill_alpha":0.5,"blur":24,"saturation":1.6,"oops":1}
    },"children":[]}}}"#;
    let err = serde_json::from_str::<Description>(json).unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{err}");
}
