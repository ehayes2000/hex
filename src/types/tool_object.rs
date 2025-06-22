use super::Tool;
use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use schemars::JsonSchema;
use schemars::schema::{RootSchema, Schema, SchemaObject};
use schemars::schema_for;
use serde::de::Deserialize;
use serde_json::Error as JsonError;
use serde_json::Value;
use thiserror::Error;

type Deserializer<T> = Box<dyn Fn(&str) -> Result<Box<dyn Tool<Context = T>>, serde_json::Error>>;
pub struct ToolObject<T> {
    pub schema: RootSchema,
    pub json_schema: Value,
    pub description: String,
    pub name: String,
    deserializer: Deserializer<T>,
}

impl<T> ToolObject<T> {
    pub fn try_deserialize(
        &self,
        data: &str,
    ) -> Result<Box<dyn Tool<Context = T>>, serde_json::Error> {
        let deserializer = &self.deserializer;
        deserializer(data)
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("missing missing metadata")]
    MissingMetadata,
    #[error("could not convert to json")]
    JsonSerialization(JsonError),
}

impl<T> From<&ToolObject<T>> for ChatCompletionTool {
    fn from(value: &ToolObject<T>) -> Self {
        Self {
            r#type: ChatCompletionToolType::Function,
            function: FunctionObject {
                name: value.name.clone(),
                description: Some(value.description.clone()),
                parameters: Some(value.json_schema.clone()),
                strict: None,
            },
        }
    }
}

impl<C> ToolObject<C> {
    pub fn try_from_tool<T>() -> Result<Self, ValidationError>
    where
        T: JsonSchema + Tool<Context = C> + for<'de> Deserialize<'de> + 'static,
    {
        let schema = schema_for!(&T);

        let (name, description) = validate_tool_schema(&schema.schema)?;

        let json_schema =
            serde_json::to_value(schema.clone()).map_err(ValidationError::JsonSerialization)?;

        let deserializer = Box::new(|data: &str| {
            serde_json::from_str::<T>(data).map(|tool| Box::new(tool) as Box<dyn Tool<Context = C>>)
        });

        Ok(Self {
            name,
            json_schema,
            schema,
            description,
            deserializer,
        })
    }
}

fn validate_tool_schema(schema: &SchemaObject) -> Result<(String, String), ValidationError> {
    let name = schema
        .metadata
        .as_deref()
        .ok_or(ValidationError::MissingMetadata)?
        .title
        .as_deref()
        .ok_or(ValidationError::MissingMetadata)?
        .to_string();

    let description = validate_tool_description(schema)?;
    Ok((name, description))
}
// this could probably be converted to a compile-time check with a macro
fn validate_tool_description(schema: &SchemaObject) -> Result<String, ValidationError> {
    // validate description on subschema
    let description = schema
        .metadata
        .as_deref()
        .ok_or(ValidationError::MissingMetadata)?
        .description
        .as_deref()
        .ok_or(ValidationError::MissingMetadata)?;

    if let Some(object) = schema.object.as_deref() {
        for sub_schema in object.properties.values() {
            if let Schema::Object(sub_schema_object) = sub_schema {
                validate_tool_description(sub_schema_object)?;
            }
        }
    }

    Ok(description.to_string())
}
