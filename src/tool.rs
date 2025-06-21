use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use schemars::JsonSchema;
use schemars::schema::{RootSchema, Schema, SchemaObject};
use schemars::schema_for;
use serde::de::Deserialize;
use serde_json::Error as JsonError;
use serde_json::Value;
use std::collections::hash_map::HashMap;
use thiserror::Error;

pub trait Tool {
    fn apply(&self) -> String;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("missing description")]
    MissingDescription,
    #[error("could not convert to json")]
    JsonSerialization(JsonError),
}

type Deserializer = Box<dyn Fn(&str) -> Result<Box<dyn Tool>, serde_json::Error>>;
pub struct ToolObject {
    pub schema: RootSchema,
    pub json_schema: Value,
    pub description: String,
    pub tool: Box<dyn Tool>,
    deserializer: Deserializer,
}

impl ToolObject {
    pub fn try_deserialize(&self, data: &str) -> Result<Box<dyn Tool>, serde_json::Error> {
        let deserializer = &self.deserializer;
        deserializer(data)
    }
}

impl From<&ToolObject> for ChatCompletionTool {
    fn from(value: &ToolObject) -> Self {
        Self {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: value.tool.name().to_string(),
                description: Some(value.description.clone()),
                parameters: Some(value.json_schema.clone()),
                strict: None,
            },
        }
    }
}

impl ToolObject {
    pub fn try_from_tool<T>(tool: T) -> Result<Self, ValidationError>
    where
        T: JsonSchema + Tool + for<'de> Deserialize<'de> + 'static,
    {
        let schema = schema_for!(&T);
        let tool = Box::new(tool);

        let description = validate_tool_schema(&schema.schema)?;

        let json_schema =
            serde_json::to_value(schema.clone()).map_err(ValidationError::JsonSerialization)?;

        let deserializer = Box::new(|data: &str| {
            serde_json::from_str::<T>(data).map(|tool| Box::new(tool) as Box<dyn Tool>)
        });

        Ok(Self {
            tool,
            json_schema,
            schema,
            description,
            deserializer,
        })
    }
}

#[derive(Debug, Error)]
pub enum ToolSetError {
    #[error("error validating schema")]
    Validation(ValidationError),
    #[error("two or more tools have the same name")]
    NameConflict(&'static str),
}

#[derive(Debug, Error)]
pub enum ToolCallError {
    #[error("error deserializing tool call (possible hallucination)")]
    Deserialization(serde_json::Error),
    #[error("tool not in toolset")]
    NotFound(String),
}

pub struct ToolSet {
    tools: HashMap<&'static str, ToolObject>,
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn add_tool<T>(mut self, tool: T) -> Result<Self, ToolSetError>
    where
        T: JsonSchema + Tool + for<'de> Deserialize<'de> + 'static,
    {
        let name = tool.name();
        if self.tools.contains_key(name) {
            Err(ToolSetError::NameConflict(name))
        } else {
            let tool_object = ToolObject::try_from_tool(tool).map_err(ToolSetError::Validation)?;
            self.tools.insert(name, tool_object);
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

// this could probably be converted to a compile-time check with a macro
fn validate_tool_schema(schema: &SchemaObject) -> Result<String, ValidationError> {
    // validate description on subschema
    let description = schema
        .metadata
        .as_deref()
        .ok_or(ValidationError::MissingDescription)?
        .description
        .as_deref()
        .ok_or(ValidationError::MissingDescription)?;

    if let Some(object) = schema.object.as_deref() {
        for sub_schema in object.properties.values() {
            if let Schema::Object(sub_schema_object) = sub_schema {
                validate_tool_schema(sub_schema_object)?;
            }
        }
    }

    Ok(description.to_string())
}
