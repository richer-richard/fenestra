//! The fenestra MCP server: tools to render, inspect, interact with, and
//! verify native UIs described as `fenestra/1` JSON, exposed over the Model
//! Context Protocol. The binary serves them over stdio; this library holds
//! the tools so tests (and embedders) can drive them directly. See
//! `server::tests::tool_list_has_all_tools_with_schemas` for the current,
//! authoritative tool count.

pub mod content;
pub mod server;
