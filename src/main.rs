mod client;
mod offline_tools;
mod types;

use client::cli::CliClient;
use offline_tools::offline_toolset;
use types::{Tool, ToolSet};

use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let mut cli_client = CliClient::new(offline_toolset());
    cli_client.chat().await?;
    Ok(())
}
