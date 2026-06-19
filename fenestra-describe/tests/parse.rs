//! Unit tests for the `Description` format, parser, and validation.

use fenestra_core::{Theme, oklch};
use fenestra_describe::color::{COLOR_ROLES, resolve_color};
use fenestra_describe::format::{ColorSpec, Description, OklchColor};

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
