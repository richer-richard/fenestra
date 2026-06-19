//! The fenestra MCP server: eight tools that render and verify native UIs
//! described as `fenestra/1` JSON, exposed over the Model Context Protocol. The
//! binary serves them over stdio; this library holds the tools so tests (and
//! embedders) can drive them directly.

pub mod content;
pub mod server;
