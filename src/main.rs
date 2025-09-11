mod client;
mod mcp_client;
mod offline_tools;
mod types;

use client::mcp::client::McpEnabledClient;

use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting MCP-enabled Hex client...");
    let mut mcp_client = McpEnabledClient::new().await?;
    println!("Created mcp client");
    mcp_client.chat().await?;
    Ok(())
}
