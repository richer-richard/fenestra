//! Unit tests for the `Description` format, parser, and validation.

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame, oklch};
use fenestra_describe::color::{COLOR_ROLES, resolve_color};
use fenestra_describe::format::{ColorSpec, Description, OklchColor};
use fenestra_describe::parse::{to_element, to_element_lenient, validate};

/// Builds a frame from an element and returns its aria snapshot (no GPU).
fn light_yaml(el: &Element<String>) -> String {
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
