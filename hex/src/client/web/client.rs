use crate::types::AsyncToolSet;

use anyhow::Result;
use async_openai::Client;
use async_openai::config::Config;
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
use std::marker::PhantomData;
use std::pin::Pin;

const MAX_RECURSIONS: u32 = 10;

#[derive(Debug, Clone)]
pub enum StreamPart {
    Content(String),
    ToolCall(ToolCall),
}

struct ProcessedStream {
    pub is_tool_calls: bool,
    pub new_messages: Vec<ChatCompletionRequestMessage>,
}

#[derive(Debug, Clone, Default)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub json: String,
}

pub type ChatCompletionStream<'a> =
    Pin<Box<dyn Stream<Item = Result<StreamPart, anyhow::Error>> + 'a>>;

pub struct WebClient<'a, C: Config, T: Clone> {
    inner: Client<C>,
    toolset: AsyncToolSet<T>,
    messages: Vec<ChatCompletionRequestMessage>,
    context: T,
    phantom: PhantomData<&'a ()>,
}

impl<'a, T: Clone> WebClient<'a, OpenAIConfig, T> {
    pub fn new(toolset: AsyncToolSet<T>, context: T) -> WebClient<'a, OpenAIConfig, T> {
        let client = Client::new();
        WebClient {
            inner: client,
            toolset,
            messages: vec![],
            context,
            phantom: PhantomData,
        }
    }
}

impl<'a, C: Config, T: Clone> WebClient<'a, C, T> {
    pub async fn send_message(&'a mut self, message: String) -> Result<ChatCompletionStream<'a>> {
        self.messages.push(ChatCompletionRequestMessage::User(
            ChatCompletionRequestUserMessage {
                content: ChatCompletionRequestUserMessageContent::Text(message),
                name: None,
            },
        ));
        self.make_chat_completion_stream().await
    }

    async fn make_chat_completion_stream(&'a mut self) -> Result<ChatCompletionStream<'a>> {
        let item_stream = stream! {
                let mut stream_parts = vec![];
                for _ in 0..MAX_RECURSIONS {
                    let mut stream = match self.make_openai_chat_completion_stream()
                        .await
                        .map(Self::map_stream) {
                            Ok(stream) => stream,
                            Err(err) => {
                                yield Err(err);
                                break;
                            }
                    };

                    // consume stream
                    // accumulate to stream_parts
                    while let Some(item) = stream.next().await {
                        if item.is_err() {
                            yield item;
                            break;
                        }
                        let stream_part = item.unwrap();
                        yield Ok(stream_part.clone());
                        stream_parts.push(stream_part);
                    }
                    // call tools, aggregate response to a new request
                    let mut processed = self.process_stream_parts(stream_parts).await;
                    self.messages.append(&mut processed.new_messages);
                    // if there are no tool calls, then done
                    if !processed.is_tool_calls {
                        break
                    }
                    stream_parts = vec![];
            }
        };
        Ok(Box::pin(item_stream))
    }

    async fn process_stream_parts(&self, stream_parts: Vec<StreamPart>) -> ProcessedStream {
        let mut tool_calls = vec![];
        let mut tool_responses = vec![];
        let mut response = String::new();
        let mut is_tool_calls = false;
        for item in stream_parts {
            match item {
                StreamPart::ToolCall(call) => {
                    is_tool_calls = true;
                    if let Ok(response) = self
                        .toolset
                        .try_tool_call(self.context.clone(), &call.name, &call.json)
                        .await
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
                        if let Ok(text) = response {
                            tool_responses.push(ChatCompletionRequestMessage::Tool(
                            ChatCompletionRequestToolMessage {
                              content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(text),
                              tool_call_id: call.id
                            },
                        ));
                        } else {
                            tool_responses.push(ChatCompletionRequestMessage::Tool(
                                ChatCompletionRequestToolMessage {
                                  content: async_openai::types::ChatCompletionRequestToolMessageContent::Text(
                                    "tool call failed".to_string()
                                  ),
                                  tool_call_id: call.id
                                },
                            ));
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
        ProcessedStream {
            is_tool_calls,
            new_messages: messages,
        }
    }

    fn map_stream(mut stream: ChatCompletionResponseStream) -> ChatCompletionStream<'a> {
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

    async fn make_openai_chat_completion_stream(&mut self) -> Result<ChatCompletionResponseStream> {
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
}
