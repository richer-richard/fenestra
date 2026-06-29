//! The formal JSON Schema for the `fenestra/1` Description format — the
//! machine-checkable companion to the prose vocabulary.

use fenestra_describe::format::description_schema;
use fenestra_describe::vocabulary::describe_vocabulary;

#[test]
fn schema_is_a_nontrivial_object_for_the_description() {
    let schema = description_schema();
    assert!(schema.is_object(), "the schema is a JSON object");
    let txt = serde_json::to_string(&schema).unwrap();
    assert!(txt.len() > 1000, "the schema is substantial: {} bytes", txt.len());
    // The root carries the description's own fields.
    assert!(txt.contains("\"root\""), "the `root` field is present: {txt:.0}");
    assert!(txt.contains("\"schema\""), "the `schema` field is present");
}

#[test]
fn schema_covers_every_vocabulary_node() {
    // Drift guard: the formal schema (derived from the format types) and the prose
    // vocabulary (from the node registry) must list the same authorable nodes, so a
    // client validating against the schema accepts exactly what the parser does.
    let txt = serde_json::to_string(&description_schema()).unwrap();
    for node in describe_vocabulary().nodes {
        assert!(
            txt.contains(&format!("\"{}\"", node.tag)),
            "the schema is missing the node tag {:?}",
            node.tag
        );
    }
}
