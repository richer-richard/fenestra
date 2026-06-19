# fenestra-mcp

A [Model Context Protocol](https://modelcontextprotocol.io) server that lets an
AI assistant render and verify native UIs described as
[fenestra](https://github.com/richer-richard/fenestra) `fenestra/1` JSON.

## Install

```sh
cargo install fenestra-mcp
```

The server speaks MCP over stdio — point your MCP client at the `fenestra-mcp`
binary.

## Tools

- `describe_vocabulary` — the description grammar (call this first).
- `render_ui` — render to a typed access tree, a preview image, and a11y warnings.
- `query_ui` — find nodes by a semantic selector.
- `interact` — drive clicks / typing / keys by selector (never coordinates).
- `check_a11y` — contrast, labeling, and per-text-node legibility.
- `match_aria_snapshot` — assert an accessibility snapshot.
- `match_screenshot` — compare against a baseline image.
- `validate` — validate a description without rendering.

Each tool leads with a typed structured result; the visual tools also attach a
downscaled preview image, and write the full-resolution PNG to a temp file whose
path is returned in the structured result.

## License

Licensed under either of MIT or Apache-2.0 at your option.
