mod list_directory;
mod read_file;

use super::ToolSet;

use list_directory::ListDirectory;
use read_file::ReadFiles;

pub fn offline_toolset() -> ToolSet {
    ToolSet::new()
        .add_tool::<ListDirectory>()
        .expect("list directory")
        .add_tool::<ReadFiles>()
        .expect("read file")
}
