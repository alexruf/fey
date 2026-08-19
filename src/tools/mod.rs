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

/// Splits `WorkspaceError` into model-actionable failures (the model can retry
/// with a corrected path) and host-only failures (`from_error`, not
/// `other(err.to_string())`, so only stable kind-level feedback reaches the
/// model). See docs/decisions/0005-tool-error-visibility.md. The model-visible
/// arms below rely on `WorkspaceError`'s variants themselves carrying only the
/// workspace-relative path the model sent, never this machine's absolute path
/// (see `Workspace::resolve` in `src/workspace.rs`).
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use rig::tool::ToolErrorKind;

    use super::*;

    /// A path with no plausible relation to a real workspace root, standing
    /// in for "the absolute path on the host machine" in assertions below.
    fn host_only_path() -> PathBuf {
        PathBuf::from("/host/machine/only/secret.txt")
    }

    #[test]
    fn empty_path_is_model_visible_and_invalid_args() {
        let error = map_workspace_error(WorkspaceError::EmptyPath);

        assert_eq!(error.kind(), ToolErrorKind::InvalidArgs);
        assert_eq!(error.model_feedback(), Some("path must not be empty"));
    }

    #[test]
    fn wrong_kind_is_model_visible_and_carries_no_host_path() {
        let requested = PathBuf::from("sub/file.txt");
        let error = map_workspace_error(WorkspaceError::WrongKind {
            path: requested.clone(),
            expected: "directory",
            actual: "file",
        });

        assert_eq!(error.kind(), ToolErrorKind::InvalidArgs);
        let feedback = error.model_feedback().expect("feedback present");
        assert!(feedback.contains("sub/file.txt"));
        assert!(!feedback.contains(host_only_path().to_str().unwrap()));
    }

    #[test]
    fn not_found_is_model_visible_with_not_found_kind() {
        let error = map_workspace_error(WorkspaceError::NotFound(PathBuf::from("missing.txt")));

        assert_eq!(error.kind(), ToolErrorKind::NotFound);
        assert_eq!(
            error.model_feedback(),
            Some("path not found: \"missing.txt\"")
        );
    }

    #[test]
    fn outside_workspace_is_a_model_visible_refusal() {
        let error = map_workspace_error(WorkspaceError::OutsideWorkspace(PathBuf::from(
            "../escape.txt",
        )));

        assert!(error.is_refusal());
        let feedback = error.model_feedback().expect("feedback present");
        assert!(feedback.contains("../escape.txt"));
    }

    /// I4 (docs/architecture.md): host-only failures must reach the model as
    /// generic, stable feedback — never the operator diagnostic, which may
    /// carry this machine's absolute path. See ADR-0005.
    #[test]
    fn host_only_failures_redact_the_diagnostic_from_model_feedback() {
        let path = host_only_path();
        let io_error = WorkspaceError::Io {
            path: path.clone(),
            source: io::Error::other("disk exploded"),
        };
        let operator_message = io_error.to_string();

        let error = map_workspace_error(io_error);

        assert_eq!(error.kind(), ToolErrorKind::Other);
        let feedback = error.model_feedback().expect("feedback present");
        assert_eq!(feedback, "the tool failed");
        assert!(!feedback.contains(path.to_str().unwrap()));
        // The full diagnostic — including the host path — is still available
        // to whoever's running fey, just not sent to the model.
        assert_eq!(error.message(), operator_message);
    }

    #[test]
    fn too_large_is_host_only_and_redacted() {
        let error = map_workspace_error(WorkspaceError::TooLarge {
            path: host_only_path(),
            max: 1024,
        });

        assert_eq!(error.kind(), ToolErrorKind::Other);
        assert_eq!(error.model_feedback(), Some("the tool failed"));
    }

    #[test]
    fn not_utf8_is_host_only_and_redacted() {
        let error = map_workspace_error(WorkspaceError::NotUtf8(host_only_path()));

        assert_eq!(error.kind(), ToolErrorKind::Other);
        assert_eq!(error.model_feedback(), Some("the tool failed"));
    }
}
