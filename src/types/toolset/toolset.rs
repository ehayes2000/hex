use super::tool_object::ToolObject;
use super::types::*;
use crate::types::Tool;
use async_openai::types::ChatCompletionTool;
use schemars::JsonSchema;
use serde::de::Deserialize;
use std::collections::hash_map::HashMap;

#[derive(Default)]
pub struct ToolSet<T> {
    tools: HashMap<String, ToolObject<T>>,
}

impl<C> ToolSet<C> {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn add_tool<T>(mut self) -> Result<Self, ToolSetCreationError>
    where
        T: JsonSchema + Tool<Context = C> + for<'de> Deserialize<'de> + 'static,
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

    pub fn try_tool_call(
        &self,
        context: C,
        tool_name: &str,
        json: &str,
    ) -> Result<String, ToolCallError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ToolCallError::NotFound(tool_name.to_owned()))
            .and_then(|tool| {
                tool.try_deserialize(json)
                    .map_err(ToolCallError::Deserialization)
            })?;
        Ok(tool.apply(context))
    }
}

impl<T> ToolSet<T> {
    pub fn openai_chatcompletion_toolset(&self) -> Vec<ChatCompletionTool> {
        self.tools.values().map(ChatCompletionTool::from).collect()
    }
}
