//! The formal JSON Schema for the `fenestra/1` Description format — the
//! machine-checkable companion to the prose vocabulary.

use fenestra_describe::format::description_schema;
use fenestra_describe::vocabulary::describe_vocabulary;

#[test]
fn schema_is_a_nontrivial_object_for_the_description() {
    let schema = description_schema();
    assert!(schema.is_object(), "the schema is a JSON object");
    let txt = serde_json::to_string(&schema).unwrap();
    assert!(
        txt.len() > 1000,
        "the schema is substantial: {} bytes",
        txt.len()
    );
    // The root carries the description's own fields.
    assert!(txt.contains("\"root\""), "the `root` field is present");
    assert!(txt.contains("\"schema\""), "the `schema` field is present");
}

#[test]
fn schema_node_variants_match_the_vocabulary_exactly() {
    // Drift guard, structural and BOTH directions: the externally-tagged `Node`
    // oneOf in the schema and the vocabulary's node list must be the same set, so a
    // client validating against the schema accepts exactly the nodes the parser does
    // — and neither side can gain or lose a node without the other. Reading the
    // oneOf tags is exact; a substring search would be one-directional and could
    // collide with field keys like `status`.
    let schema = description_schema();
    let variants = schema["$defs"]["Node"]["oneOf"]
        .as_array()
        .expect("Node is a oneOf of tagged variants");
    let schema_tags: std::collections::BTreeSet<&str> = variants
        .iter()
        .map(|v| {
            v["required"][0]
                .as_str()
                .expect("each Node variant requires exactly its tag key")
        })
        .collect();
    let vocab = describe_vocabulary();
    let vocab_tags: std::collections::BTreeSet<&str> =
        vocab.nodes.iter().map(|n| n.tag.as_str()).collect();
    assert_eq!(
        schema_tags, vocab_tags,
        "the schema's Node variants and the vocabulary's nodes must be the same set"
    );
}
