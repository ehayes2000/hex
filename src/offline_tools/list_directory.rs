use crate::Tool;
use crate::types::NoContext;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "list the files in a directory")]
pub struct ListDirectory {
    #[schemars(description = "file path to list")]
    pub path: String,
}

impl Tool for ListDirectory {
    type Context = NoContext;
    fn apply(&self, _: Self::Context) -> String {
        let contents = std::fs::read_dir(&self.path);
        if contents.is_err() {
            return format!("could not list {}", self.path);
        }
        let contents = contents.unwrap();
        contents
            .into_iter()
            .filter_map(|path| match path {
                Ok(entry) => entry.path().to_str().map(str::to_string),
                Err(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
