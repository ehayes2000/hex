use crate::types::NoContext;
use crate::types::ToolSet;

use anyhow::{Context, Result};
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
use std::pin::Pin;

pub struct WebClient<C: Config> {
    inner: Client<C>,
    toolset: ToolSet<NoContext>,
    messages: Vec<ChatCompletionRequestMessage>,
}
