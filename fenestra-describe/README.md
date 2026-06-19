# fenestra-describe

Serialized UI for [fenestra](https://github.com/richer-richard/fenestra): a JSON
`Description` parses to the same `Element` tree the builders produce, then runs
through the identical render and verification pipeline.

The crate is windowless — it depends only on `fenestra-core` and `fenestra-kit`,
so parsing, the access tree, semantic queries, aria snapshots, and accessibility
checks all run without a GPU. Pixel rendering lives one layer up, in
[`fenestra-cli`](https://crates.io/crates/fenestra-cli).

## What's here

- **`Description`** — a schema-tagged (`"fenestra/1"`), strict
  (`deny_unknown_fields`) JSON mirror of a UI: containers (`row`/`col`/`div`/
  `stack`), `text`, and the interactive widgets (`button`, `checkbox`, `switch`,
  `radio`, `slider`, `text_input`, `text_area`). Colors are theme role names or
  an `oklch` escape hatch; handlers are inert intent strings.
- **`parse::to_element`** — converts a `Description` to an `Element<String>`,
  degrading gracefully (an unresolvable color becomes a default plus a
  path-pointed error) rather than panicking.
- **`parse::validate`** — checks a description without rendering; structural and
  semantic problems come back path-pointed.
- **`inspect`** — the typed access tree, semantic `query`, `aria_snapshot` +
  `match_aria` (partial / strict / regex), and `check_a11y` (theme contrast,
  labeling, and per-text-node APCA + WCAG 2 legibility).
- **`vocabulary::describe_vocabulary`** — the grammar, generated from the same
  registry the parser uses.

## Example

```rust
use fenestra_core::Theme;
use fenestra_describe::format::Description;
use fenestra_describe::inspect::access_tree;

let desc: Description = serde_json::from_str(
    r#"{ "schema": "fenestra/1", "root": { "col": { "children": [
        { "text": { "content": "Hello" } },
        { "button": { "label": "Go", "on_click": "go" } }
    ] } } }"#,
)
.unwrap();

let tree = access_tree(&desc, &Theme::light(), (320, 200)).unwrap();
assert!(tree.children.iter().any(|n| n.role == "button"));
```

## License

Licensed under either of MIT or Apache-2.0 at your option.
