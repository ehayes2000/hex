use super::io::{read_user_input, stdout_stream};

use crate::types::NoContext;
use crate::types::SyncToolSet;

use anyhow::{Context, Result};
use async_openai::Client;
use async_openai::config::OpenAIConfig;
use async_openai::types::{
    ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessage,
    ChatCompletionRequestAssistantMessageContent, ChatCompletionRequestMessage,
    ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage,
    ChatCompletionRequestUserMessageContent, ChatCompletionResponseStream,
    CreateChatCompletionRequestArgs, FinishReason, FunctionCall,
};
use async_stream::stream;
use futures::stream::{Stream, StreamExt};
use std::collections::HashMap;
use std::pin::Pin;

pub struct CliClient {
    inner: Client<OpenAIConfig>,
    toolset: SyncToolSet<NoContext>,
    messages: Vec<ChatCompletionRequestMessage>,
}

#[derive(Debug, Default)]
pub struct ToolCal {
    pub id: String,
    pub name: String,
    pub json: String,
}

#[derive(Debug)]
pub enum StreamPart {
    Content(String),
    ToolCall(ToolCal),
}

impl CliClient {
    pub fn new(toolset: SyncToolSet<NoContext>) -> CliClient {
        let client = Client::new();
        CliClient {
            inner: client,
            toolset,
            messages: vec![],
        }
    }

    pub async fn chat(&mut self) -> Result<()> {
        print!("\x1B[2J\x1B[1;1H");
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
}

impl CliClient {
    async fn chat_response(&mut self) -> Result<()> {
        let stream = self.send_chat_message().await.context("bad request")?;
        let stream = Self::parse_stream(stream);
        let contents = stdout_stream(stream).await?;
        let mut new_messages = self.process_stream(contents);
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
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4.1")
            .messages(self.messages.clone())
            .tools(self.toolset.openai_chatcompletion_toolset())
            .n(1)
            .build()?;

        self.inner
            .chat()
            .create_stream(request)
            .await
            .map_err(anyhow::Error::from)
    }

    fn parse_stream(
        mut stream: ChatCompletionResponseStream,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamPart>>>> {
        Box::pin(stream! {
        let mut tool_calls: HashMap<u32, ToolCal> = HashMap::new();
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
                        let mut partial = ToolCal::default();
                        if let Some(n) = function.name.clone() {
                          partial.name = n;
                        }
                        if let Some(a) = function.arguments.clone() {
                          partial.json = a;
                        }
                        if let Some(id) = &call.id {
                          partial.id = id.clone();
                        }
                        // if let Some(id) = function.id
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

    fn process_stream(&self, items: Vec<StreamPart>) -> Vec<ChatCompletionRequestMessage> {
        let mut tool_calls = vec![];
        let mut tool_responses = vec![];
        let mut response = String::new();
        for item in items {
            match item {
                StreamPart::ToolCall(call) => {
                    if let Ok(context) = self
                        .toolset
                        .try_tool_call(NoContext(), &call.name, &call.json)
                        .inspect_err(|err| eprintln!("error: {:?}", err))
                    {
                        tool_calls.push(ChatCompletionMessageToolCall {
                            id: call.id.clone(),
                            r#type: async_openai::types::ChatCompletionToolType::Function,
                            function: FunctionCall {
                                arguments: call.json,
                                name: call.name,
                            },
                        });
                        tool_responses.push(ChatCompletionRequestMessage::Tool(
                            ChatCompletionRequestToolMessage {
                              content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(context),
                              tool_call_id: call.id
                            },
                        ));
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
