mod cli_client;
mod io;
mod offline_tools;
mod tool;

use offline_tools::offline_toolset;
use tool::{Tool, ToolSet};

use anyhow::Result;
use async_openai::types::{ChatCompletionRequestUserMessageArgs, ChatCompletionToolChoiceOption};
use async_openai::{Client, types::CreateChatCompletionRequestArgs};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Default)]
#[schemars(description = "open and edit a file with an AI agent")]
pub struct EditFile {
    #[schemars(description = "the file id to edit")]
    pub file_id: String,
    #[schemars(description = "a description of the edits the user would like to make")]
    pub edit_description: String,
}

impl Tool for EditFile {
    fn apply(&self) -> String {
        "edited file real good".to_string()
    }
    fn name(&self) -> &'static str {
        "edit_file"
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut cli_client = cli_client::CliClient::new(offline_toolset());
    cli_client.chat().await?;
    Ok(())
}
