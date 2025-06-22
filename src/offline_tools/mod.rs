mod create_file;
mod edit_file;
mod list_directory;
mod read_file;

use crate::types::NoContext;
use crate::types::SyncToolSet;

use create_file::CreateFile;
use edit_file::EditFile;
use list_directory::ListDirectory;
use read_file::ReadFiles;

pub fn offline_toolset() -> SyncToolSet<NoContext> {
    SyncToolSet::new()
        .add_tool::<ListDirectory>()
        .expect("list directory")
        .add_tool::<ReadFiles>()
        .expect("read file")
        .add_tool::<EditFile>()
        .expect("edit file")
        .add_tool::<CreateFile>()
        .expect("create file")
}
