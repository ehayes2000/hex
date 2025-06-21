mod cli_client;
mod io;
mod offline_tools;
mod tool;

use offline_tools::offline_toolset;
use tool::{Tool, ToolSet};

use anyhow::Result;
use async_openai::types::{ChatCompletionRequestUserMessageArgs, ChatCompletionToolChoiceOption};
use async_openai::{types::CreateChatCompletionRequestArgs, Client};
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

#[allow(unused)]
async fn toolset_test() -> Result<()> {
    let toolset = ToolSet::new().add_tool(EditFile::default())?;

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4.1")
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(
                "please open 'RFD 3 - Tool Usage' and change rewrite all of the code blocks in js",
            )
            .build()?
            .into()])
        .tools(toolset.openai_chatcompletion_toolset())
        .n(1)
        .tool_choice(ChatCompletionToolChoiceOption::Required)
        .build()?;

    let client = Client::new();

    let response = client.chat().create(request).await?;

    let tool_call = response
        .choices
        .first()
        .expect("one choice")
        .to_owned()
        .message
        .to_owned()
        .tool_calls
        .expect("tool calls")
        .first()
        .expect("one tool call")
        .to_owned();

    let deserialized_tool = serde_json::from_str::<EditFile>(&tool_call.function.arguments)
        .expect("deserialize tool call");

    println!("{:#?}", deserialized_tool);
    Ok(())
}
