use crate::types::{AsyncTool, AsyncToolWrapper, Tool};

use async_openai::types::{ChatCompletionTool, ChatCompletionToolType, FunctionObject};
use schemars::JsonSchema;
use schemars::schema::{Schema, SchemaObject};
use schemars::schema_for;
use serde::de::Deserialize;
use serde_json::Error as JsonError;
use serde_json::Value;
use thiserror::Error;

type ToolTraitObject<T> = Box<dyn Tool<Context = T> + Send + Sync>;
type Deserializer<T> = Box<dyn Fn(&str) -> Result<ToolTraitObject<T>, serde_json::Error>>;

type AsyncToolTraitObject<T> = Box<dyn AsyncTool<Context = T>>;
type AsyncDeserializer<T> = Box<dyn Fn(&str) -> Result<AsyncToolTraitObject<T>, serde_json::Error>>;

pub type SyncToolObject<Context> = ToolObject<Deserializer<Context>>;
pub type AsyncToolObject<Context> = ToolObject<AsyncDeserializer<Context>>;

pub struct ToolObject<T> {
    pub json_schema: Value,
    pub description: String,
    pub name: String,
    deserializer: T,
}

impl<C> ToolObject<Deserializer<C>> {
    pub fn try_deserialize(&self, data: &str) -> Result<ToolTraitObject<C>, serde_json::Error> {
        let deserializer = &self.deserializer;
        deserializer(data)
    }
}

impl<C> ToolObject<AsyncDeserializer<C>> {
    pub fn try_deserialize(
        &self,
        data: &str,
    ) -> Result<AsyncToolTraitObject<C>, serde_json::Error> {
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

impl<C> ToolObject<Deserializer<C>> {
    pub fn try_from_tool<T>() -> Result<Self, ValidationError>
    where
        T: JsonSchema + Tool<Context = C> + Send + Sync + for<'de> Deserialize<'de> + 'static,
    {
        let schema = schema_for!(&T);

        let (name, description) = validate_tool_schema(&schema.schema)?;

        let json_schema =
            serde_json::to_value(schema.clone()).map_err(ValidationError::JsonSerialization)?;

        let deserializer = Box::new(|data: &str| {
            serde_json::from_str::<T>(data)
                .map(|tool| Box::new(tool) as Box<dyn Tool<Context = C> + Send + Sync>)
        });

        Ok(Self {
            name,
            json_schema,
            description,
            deserializer,
        })
    }
}

impl<C> ToolObject<AsyncDeserializer<C>> {
    pub fn try_from_tool<T>() -> Result<Self, ValidationError>
    where
        T: JsonSchema + AsyncTool<Context = C> + for<'de> Deserialize<'de> + 'static,
    {
        let schema = schema_for!(&T);

        let (name, description) = validate_tool_schema(&schema.schema)?;

        let json_schema =
            serde_json::to_value(schema.clone()).map_err(ValidationError::JsonSerialization)?;

        let deserializer = Box::new(|data: &str| {
            serde_json::from_str::<T>(data)
                .map(|tool| Box::new(tool) as Box<dyn AsyncTool<Context = C>>)
        });

        Ok(Self {
            name,
            json_schema,
            description,
            deserializer,
        })
    }
}

impl<C> From<SyncToolObject<C>> for AsyncToolObject<C>
where
    C: Send + Sync + 'static,
{
    fn from(value: SyncToolObject<C>) -> Self {
        let async_deserializer = Box::new(move |json: &str| {
            (value.deserializer)(json).map(|trait_obj| {
                Box::new(AsyncToolWrapper { tool: trait_obj }) as AsyncToolTraitObject<C>
            })
        });
        Self {
            description: value.description,
            json_schema: value.json_schema,
            name: value.name,
            deserializer: async_deserializer,
        }
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
