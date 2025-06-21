mod list_directory;
mod read_file;

use super::tool::ToolSet;

use list_directory::ListDirectory;
use read_file::ReadFiles;

pub fn offline_toolset() -> ToolSet {
    ToolSet::new()
        .add_tool(ListDirectory::default())
        .expect("list directory")
        .add_tool(ReadFiles::default())
        .expect("read file")
}
