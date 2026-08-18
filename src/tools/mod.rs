//! Rig adapter layer: thin wrappers over the plain filesystem helpers in
//! `list_directory` and `read_file`, exposed as `ListDirectory` and `ReadFile`
//! tool structs for registration with a Rig agent.

mod list_directory;
mod read_file;

use rig::tool::{ToolContext, ToolExecutionError};

use crate::workspace::{Workspace, WorkspaceError};
use list_directory::list_directory as list_directory_impl;
use read_file::{ReadFileError, read_file as read_file_impl};

fn workspace_from_context(context: &ToolContext) -> Result<&Workspace, ToolExecutionError> {
    context
        .get::<Workspace>()
        .ok_or_else(|| ToolExecutionError::other("workspace missing from tool context"))
}

fn map_workspace_error(err: WorkspaceError) -> ToolExecutionError {
    match err {
        WorkspaceError::EmptyPath
        | WorkspaceError::AbsolutePath(_)
        | WorkspaceError::WrongKind { .. } => ToolExecutionError::invalid_args(err.to_string()),
        WorkspaceError::OutsideWorkspace(_) => ToolExecutionError::refused(err.to_string()),
        WorkspaceError::NotFound(_) => ToolExecutionError::not_found(err.to_string()),
        WorkspaceError::NotUtf8(_)
        | WorkspaceError::TooLarge { .. }
        | WorkspaceError::Io { .. }
        | WorkspaceError::RootNotADirectory(_) => ToolExecutionError::from_error(err),
    }
}

#[rig::rig_tool(
    description = "List the immediate children of a directory inside the workspace. Paths are workspace-relative; omit path for the workspace root."
)]
pub(crate) fn list_directory(
    #[rig(context)] context: &mut ToolContext,
    path: Option<String>,
) -> Result<String, ToolExecutionError> {
    let workspace = workspace_from_context(context)?;
    list_directory_impl(workspace, path.as_deref()).map_err(map_workspace_error)
}

#[rig::rig_tool(
    description = "Read UTF-8 text from a file inside the workspace with one-based line numbers. Paths are workspace-relative; start_line defaults to 1."
)]
pub(crate) fn read_file(
    #[rig(context)] context: &mut ToolContext,
    path: String,
    start_line: Option<usize>,
) -> Result<String, ToolExecutionError> {
    let workspace = workspace_from_context(context)?;
    read_file_impl(workspace, &path, start_line).map_err(|err| match err {
        ReadFileError::ZeroStartLine => ToolExecutionError::invalid_args(err.to_string()),
        ReadFileError::Workspace(source) => map_workspace_error(source),
    })
}
