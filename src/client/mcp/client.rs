use super::super::cli::io::read_user_input;
use crate::mcp_client::{McpClient, McpServer};

use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionResponseStream,
    CreateChatCompletionRequestArgs, FinishReason, FunctionCall, ChatCompletionTool,
    ChatCompletionToolType, FunctionObject,
};
use async_stream::stream;
use futures::stream::{Stream, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use tokio::io::AsyncWriteExt;

pub struct McpEnabledClient {
    openai: Client<OpenAIConfig>,
    mcp_client: McpClient,
    messages: Vec<ChatCompletionRequestMessage>,
}

#[derive(Debug, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug)]
pub enum StreamPart {
    Content(String),
    ToolCall(ToolCall),
}

impl McpEnabledClient {
    pub async fn new() -> Result<Self> {
        let mut mcp_client = McpClient::new();
        
        // Add Linear MCP server using mcp-remote for HTTP/SSE transport
        let linear_server = McpServer {
            name: "linear".to_string(),
            command: "npx".to_string(),
            args: vec!["-y".to_string(), "mcp-remote".to_string(), "https://mcp.linear.app/sse".to_string()],
            env: HashMap::new(),
            tools: Vec::new(),
        };
        
        mcp_client.add_server(linear_server);
        mcp_client.initialize().await?;

        Ok(Self {
            openai: Client::new(),
            mcp_client,
            messages: vec![],
        })
    }

    pub async fn chat(&mut self) -> Result<()> {
        print!("\x1B[2J\x1B[1;1H");
        println!("MCP-enabled Hex client started!");
        println!("Available MCP tools:");
        for tool in self.mcp_client.get_all_tools() {
            println!("  - {}: {}", tool.name, tool.description);
        }
        println!();

        loop {
            let user_input = read_user_input().await?;

            self.messages.push(ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessage {
                    content: ChatCompletionRequestUserMessageContent::Text(user_input),
                    name: None,
                },
            ));

            self.chat_response().await?;
        }
    }

    async fn chat_response(&mut self) -> Result<()> {
        let stream = self.send_chat_message().await.context("bad request")?;
        let stream = Self::parse_stream(stream);
        let contents = self.mcp_stdout_stream(stream).await?;
        let mut new_messages = self.process_stream(contents).await;
        
        if new_messages
            .iter()
            .any(|message| matches!(message, ChatCompletionRequestMessage::Tool(..)))
        {
            new_messages.iter().for_each(|message| {
                if let ChatCompletionRequestMessage::Assistant(message) = message {
                    for call in message.tool_calls.as_deref().unwrap_or_default() {
                        println!("[{}({})]", call.function.name, call.function.arguments);
                    }
                }
            });
            self.messages.append(&mut new_messages);
            Box::pin(self.chat_response()).await
        } else {
            println!();
            self.messages.append(&mut new_messages);
            Ok(())
        }
    }

    async fn send_chat_message(&mut self) -> Result<ChatCompletionResponseStream> {
        // Convert MCP tools to OpenAI tool format
        let mcp_tools: Vec<ChatCompletionTool> = self
            .mcp_client
            .get_all_tools()
            .iter()
            .map(|tool| ChatCompletionTool {
                r#type: ChatCompletionToolType::Function,
                function: FunctionObject {
                    name: tool.name.clone(),
                    description: Some(tool.description.clone()),
                    parameters: Some(tool.input_schema.clone()),
                    strict: None,
                },
            })
            .collect();

        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4o")
            .messages(self.messages.clone())
            .tools(mcp_tools)
            .n(1)
            .build()?;

        self.openai
            .chat()
            .create_stream(request)
            .await
            .map_err(anyhow::Error::from)
    }

    fn parse_stream(
        mut stream: ChatCompletionResponseStream,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamPart>>>> {
        Box::pin(stream! {
            let mut tool_calls: HashMap<u32, ToolCall> = HashMap::new();
            while let Some(part) = stream.next().await {
                match part {
                    Ok(part) => {
                        let first = part.choices.first();
                        if first.is_none() {
                            continue;
                        }
                        let first = first.unwrap();
                        if let Some(content) = &first.delta.content {
                            yield Ok(StreamPart::Content(content.clone()));
                        }
                        if let Some(calls) = &first.delta.tool_calls {
                            for call in calls {
                                if let Some(function) = &call.function {
                                    tool_calls.entry(call.index)
                                        .and_modify(|partial| {
                                            if let Some(n) = &function.name {
                                                partial.name = format!("{}{}", partial.name, n);
                                            }
                                            if let Some(a) = &function.arguments.clone() {
                                                partial.json = format!("{}{}", partial.json, a);
                                            }
                                            if let Some(id) = &call.id {
                                                partial.id = id.clone();
                                            }
                                        })
                                        .or_insert_with(|| {
                                            let mut partial = ToolCall::default();
                                            if let Some(n) = function.name.clone() {
                                                partial.name = n;
                                            }
                                            if let Some(a) = function.arguments.clone() {
                                                partial.json = a;
                                            }
                                            if let Some(id) = &call.id {
                                                partial.id = id.clone();
                                            }
                                            partial
                                        });
                                }
                            }
                        }
                        if let Some(FinishReason::ToolCalls) = first.finish_reason {
                            for call in tool_calls.into_values() {
                                yield Ok(StreamPart::ToolCall(call));
                            }
                            tool_calls = HashMap::new();
                        }
                    },
                    Err(error) => yield Err(anyhow::Error::from(error))
                }
            }
        })
    }

    async fn mcp_stdout_stream(
        &self,
        mut stream: Pin<Box<dyn Stream<Item = Result<StreamPart>>>>,
    ) -> Result<Vec<StreamPart>> {
        let mut output = tokio::io::stdout();
        let mut parts = Vec::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(response) => {
                    if let StreamPart::Content(ref text) = response {
                        output.write_all(text.as_bytes()).await?
                    }
                    parts.push(response);
                }
                Err(error) => output.write_all(format!("{}", error).as_bytes()).await?,
            }
            output.flush().await?;
        }
        Ok(parts)
    }

    async fn process_stream(&self, items: Vec<StreamPart>) -> Vec<ChatCompletionRequestMessage> {
        let mut tool_calls = vec![];
        let mut tool_responses = vec![];
        let mut response = String::new();
        
        for item in items {
            match item {
                StreamPart::ToolCall(call) => {
                    // Parse the JSON arguments
                    match serde_json::from_str::<Value>(&call.json) {
                        Ok(args) => {
                            // Call the MCP tool
                            match self.mcp_client.call_tool(&call.name, args).await {
                                Ok(result) => {
                                    tool_calls.push(ChatCompletionMessageToolCall {
                                        id: call.id.clone(),
                                        r#type: ChatCompletionToolType::Function,
                                        function: FunctionCall {
                                            arguments: call.json,
                                            name: call.name,
                                        },
                                    });
                                    tool_responses.push(ChatCompletionRequestMessage::Tool(
                                        ChatCompletionRequestToolMessage {
                                            content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(result),
                                            tool_call_id: call.id,
                                        },
                                    ));
                                }
                                Err(e) => {
                                    eprintln!("Error calling MCP tool {}: {:?}", call.name, e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error parsing tool arguments: {:?}", e);
                        }
                    }
                }
                StreamPart::Content(text) => response.push_str(text.as_str()),
            };
        }
        
        let assistant_response =
            ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
                content: if response.is_empty() {
                    None
                } else {
                    Some(ChatCompletionRequestAssistantMessageContent::Text(response))
                },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                ..Default::default()
            });
        
        let mut messages = vec![assistant_response];
        messages.append(&mut tool_responses);
        messages
    }
}