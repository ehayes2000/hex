mod cli_client;
mod io;
mod offline_tools;
mod tool;

use offline_tools::offline_toolset;

use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let mut cli_client = cli_client::CliClient::new(offline_toolset());
    cli_client.chat().await?;
    Ok(())
}
