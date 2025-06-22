use crate::Tool;
use crate::types::NoContext;

use schemars::JsonSchema;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[schemars(description = "Edit a file by replacing its contents (read it first)")]
pub struct EditFile {
    #[schemars(description = "file path edit")]
    pub path: String,

    #[schemars(description = "the new contents of the file")]
    pub contents: String,
}

impl Tool for EditFile {
    type Context = NoContext;
    fn apply(&self, _: Self::Context) -> String {
        println!("[{}]", self.path);
        print!("Are you sure you want to overwrite this file? [y/N]: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        if let Err(e) = io::stdin().read_line(&mut input) {
            return format!("Failed to read confirmation input: {}", e);
        }

        if input.trim().eq_ignore_ascii_case("y") {
            match fs::write(&self.path, &self.contents) {
                Ok(_) => format!("File '{}' updated successfully.", self.path),
                Err(e) => format!("Failed to write to file '{}': {}", self.path, e),
            }
        } else {
            "File update cancelled.".to_string()
        }
    }
}
