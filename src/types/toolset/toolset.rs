use super::tool_object::{AsyncToolObject, SyncToolObject};
use super::types::*;
use crate::types::{AsyncTool, Tool};
use async_openai::types::ChatCompletionTool;
use schemars::schema::RootSchema;
use schemars::{JsonSchema, schema_for};
use serde::de::Deserialize;
use std::collections::hash_map::HashMap;

pub type SyncToolSet<Context> = ToolSet<SyncToolObject<Context>>;
pub type AsyncToolSet<Context> = ToolSet<AsyncToolObject<Context>>;

#[derive(Default)]
pub struct ToolSet<T> {
    pub schemas: Vec<RootSchema>,
    tools: HashMap<String, T>,
}

impl<C> ToolSet<C> {
    pub fn new() -> Self {
        Self {
            schemas: vec![],
            tools: HashMap::new(),
        }
    }
}

impl<C> ToolSet<SyncToolObject<C>> {
    pub fn add_tool<T>(mut self) -> Result<Self, ToolSetCreationError>
    where
        T: JsonSchema + Tool<Context = C> + for<'de> Deserialize<'de> + 'static + Send + Sync,
    {
        let tool_object =
            SyncToolObject::try_from_tool::<T>().map_err(ToolSetCreationError::Validation)?;
        if self.tools.contains_key(&tool_object.name) {
            Err(ToolSetCreationError::NameConflict(tool_object.name.clone()))
        } else {
            self.tools.insert(tool_object.name.clone(), tool_object);
            let schema = schema_for!(T);
            self.schemas.push(schema);
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

impl<C> ToolSet<SyncToolObject<C>>
where
    C: Send + Sync + 'static,
{
    pub fn into_async(self) -> AsyncToolSet<C> {
        AsyncToolSet {
            schemas: self.schemas,
            tools: self
                .tools
                .into_iter()
                .map(|(name, obj)| (name, AsyncToolObject::from(obj)))
                .collect(),
        }
    }
}

impl<C> ToolSet<AsyncToolObject<C>> {
    pub fn add_tool<T>(mut self) -> Result<Self, ToolSetCreationError>
    where
        T: JsonSchema + AsyncTool<Context = C> + for<'de> Deserialize<'de> + 'static,
    {
        let tool_object =
            AsyncToolObject::try_from_tool::<T>().map_err(ToolSetCreationError::Validation)?;
        if self.tools.contains_key(&tool_object.name) {
            Err(ToolSetCreationError::NameConflict(tool_object.name.clone()))
        } else {
            self.tools.insert(tool_object.name.clone(), tool_object);
            let schema = schema_for!(T);
            self.schemas.push(schema);
            Ok(self)
        }
    }

    pub async fn try_tool_call(
        &self,
        context: C,
        tool_name: &str,
        json: &str,
    ) -> Result<Result<String, anyhow::Error>, ToolCallError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ToolCallError::NotFound(tool_name.to_owned()))
            .and_then(|tool| {
                tool.try_deserialize(json)
                    .map_err(ToolCallError::Deserialization)
            })?;
        Ok(tool.apply(context).await)
    }
}

impl<T> ToolSet<T>
where
    ChatCompletionTool: for<'a> From<&'a T>,
{
    pub fn openai_chatcompletion_toolset(&self) -> Vec<ChatCompletionTool> {
        self.tools.values().map(ChatCompletionTool::from).collect()
    }
}
