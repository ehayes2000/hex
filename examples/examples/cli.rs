use hex::client::cli::CliClient;
use tools::offline_toolset;

use anyhow::Result;
#[tokio::main]
async fn main() -> Result<()> {
    let mut cli_client = CliClient::new(offline_toolset());
    cli_client.chat().await?;
    Ok(())
}
