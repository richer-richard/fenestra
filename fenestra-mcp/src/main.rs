//! The fenestra MCP server binary: serves the twelve UI render/verify tools over
//! stdio, so an MCP client (an AI assistant) can build, render, query, and
//! assert native UIs described as JSON.

use fenestra_mcp::server::FenestraServer;
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = FenestraServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
