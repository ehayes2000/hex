use super::Tool;
use super::tool_object::{ToolObject, ValidationError};
use async_openai::types::ChatCompletionTool;
use schemars::JsonSchema;
use serde::de::Deserialize;
use std::collections::hash_map::HashMap;
use thiserror::Error;

#[derive(Default)]
pub struct ToolSet {
    tools: HashMap<String, ToolObject>,
}

#[derive(Debug, Error)]
pub enum ToolSetCreationError {
    #[error("error validating schema")]
    Validation(ValidationError),
    #[error("two or more tools have the same name")]
    NameConflict(String),
}

#[derive(Debug, Error)]
pub enum ToolCallError {
    #[error("error deserializing tool call (possible hallucination)")]
    Deserialization(serde_json::Error),
    #[error("tool not in toolset")]
    NotFound(String),
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn add_tool<T>(mut self) -> Result<Self, ToolSetCreationError>
    where
        T: JsonSchema + Tool + for<'de> Deserialize<'de> + 'static,
    {
        let tool_object =
            ToolObject::try_from_tool::<T>().map_err(ToolSetCreationError::Validation)?;
        if self.tools.contains_key(&tool_object.name) {
            Err(ToolSetCreationError::NameConflict(tool_object.name.clone()))
        } else {
            self.tools.insert(tool_object.name.clone(), tool_object);
            Ok(self)
        }
    }

    pub fn try_tool_call(&self, tool_name: &str, json: &str) -> Result<String, ToolCallError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ToolCallError::NotFound(tool_name.to_owned()))
            .and_then(|tool| {
                tool.try_deserialize(json)
                    .map_err(ToolCallError::Deserialization)
            })?;
        Ok(tool.apply())
    }
}

impl ToolSet {
    pub fn openai_chatcompletion_toolset(&self) -> Vec<ChatCompletionTool> {
        self.tools.values().map(ChatCompletionTool::from).collect()
    }
}
