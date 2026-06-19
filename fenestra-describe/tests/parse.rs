//! Unit tests for the `Description` format, parser, and validation.

use fenestra_describe::format::Description;

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
