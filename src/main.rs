mod cli_client;
mod io;
mod offline_tools;
mod types;

use offline_tools::offline_toolset;
pub use types::{Tool, ToolSet};

use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let mut cli_client = cli_client::CliClient::new(offline_toolset());
    cli_client.chat().await?;
    Ok(())
}
