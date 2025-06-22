use crate::Tool;
use crate::types::NoContext;

use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(
    description = "Create a new file with the given contents. Asks for confirmation and fails if file already exists."
)]
pub struct CreateFile {
    #[schemars(description = "file path to create")]
    pub path: String,

    #[schemars(description = "the contents of the new file")]
    pub contents: String,
}

impl Tool for CreateFile {
    type Context = NoContext;

    fn apply(&self, _: Self::Context) -> String {
        let path = Path::new(&self.path);
        if path.exists() {
            return format!(
                "Error: File '{}' already exists. Creation aborted.",
                self.path
            );
        }

        println!("[{}]", self.path);
        print!("Are you sure you want to create this file? [y/N]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if let Err(e) = io::stdin().read_line(&mut input) {
            return format!("Failed to read confirmation input: {}", e);
        }

        if input.trim().eq_ignore_ascii_case("y") {
            match fs::write(&self.path, &self.contents) {
                Ok(_) => format!("File '{}' created successfully.", self.path),
                Err(e) => format!("Failed to create file '{}': {}", self.path, e),
            }
        } else {
            "File creation cancelled.".to_string()
        }
    }
}
