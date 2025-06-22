use crate::{Tool, types::NoContext};
use schemars::JsonSchema;
use serde::Deserialize;
use std::fs::read_to_string;

#[derive(Deserialize, JsonSchema, Debug, Default)]
#[schemars(description = "Read one or more files and add their contents to context")]
pub struct ReadFiles {
    #[schemars(description = "a list of relative file paths to read")]
    pub paths: Vec<String>,
}

impl Tool for ReadFiles {
    type Context = NoContext;
    fn apply(&self, _: Self::Context) -> String {
        self.paths
            .iter()
            .map(|path| match read_to_string(path) {
                Ok(content) => format!("[{path}]\n{content}"),
                Err(_) => "<failed to read file>".to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
