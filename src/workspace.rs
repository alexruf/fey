//! Filesystem sandbox: confines tool access to a canonical workspace root.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub(crate) const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// A path that has already been checked against the sandbox, together with the
/// metadata that check read.
///
/// Only `Workspace` can build one, because the fields are private to this
/// module and every constructor path runs through `resolve`. That is what makes
/// I2 (docs/architecture.md) a compiler guarantee instead of a convention a new
/// tool has to remember: a function that wants to touch the filesystem has to
/// ask for a `ResolvedPath`, and the only way to get one is to be checked.
///
/// Carrying the metadata also means the kind and size checks share the single
/// `stat` the resolution already performed, so nothing can swap the file
/// underneath between checking it and reading it.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedPath {
    path: PathBuf,
    metadata: fs::Metadata,
}

impl ResolvedPath {
    pub(crate) fn as_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Workspace {
    root: PathBuf,
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceError {
    #[error("workspace root is not a directory: {0:?}")]
    RootNotADirectory(PathBuf),

    #[error("path must not be empty")]
    EmptyPath,

    #[error("path must be relative to the workspace: {0:?}")]
    AbsolutePath(PathBuf),

    #[error("path not found: {0:?}")]
    NotFound(PathBuf),

    #[error("path escapes the workspace: {0:?}")]
    OutsideWorkspace(PathBuf),

    #[error("expected a {expected}, found a {actual}: {path:?}")]
    WrongKind {
        path: PathBuf,
        expected: &'static str,
        actual: &'static str,
    },

    #[error("file is not valid UTF-8: {0:?}")]
    NotUtf8(PathBuf),

    #[error("file exceeds the {max}-byte limit: {path:?}")]
    TooLarge { path: PathBuf, max: u64 },

    #[error("failed to access {path:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl Workspace {
    pub(crate) fn new(root: PathBuf) -> Result<Self, WorkspaceError> {
        let canonical = fs::canonicalize(&root).map_err(|source| WorkspaceError::Io {
            path: root.clone(),
            source,
        })?;
        if !canonical.is_dir() {
            return Err(WorkspaceError::RootNotADirectory(canonical));
        }
        Ok(Self { root: canonical })
    }

    fn resolve(&self, path: &str) -> Result<ResolvedPath, WorkspaceError> {
        if path.is_empty() {
            return Err(WorkspaceError::EmptyPath);
        }
        let requested = Path::new(path);
        if requested.is_absolute() {
            return Err(WorkspaceError::AbsolutePath(requested.to_path_buf()));
        }

        let candidate = self.root.join(requested);
        let canonical = fs::canonicalize(&candidate).map_err(|source| {
            if source.kind() == io::ErrorKind::NotFound {
                WorkspaceError::NotFound(requested.to_path_buf())
            } else {
                WorkspaceError::Io {
                    path: candidate.clone(),
                    source,
                }
            }
        })?;

        if !canonical.starts_with(&self.root) {
            // The canonical target is, by definition, outside the workspace
            // root and so has no workspace-relative form to report — the
            // requested path is what the model actually sent and needs to
            // correct, so report that instead (see ADR-0005 and I4 in
            // docs/architecture.md: model-visible errors must not carry
            // absolute host paths).
            return Err(WorkspaceError::OutsideWorkspace(requested.to_path_buf()));
        }

        let metadata = fs::metadata(&canonical).map_err(|source| WorkspaceError::Io {
            path: canonical.clone(),
            source,
        })?;

        Ok(ResolvedPath {
            path: canonical,
            metadata,
        })
    }

    pub(crate) fn resolve_dir(&self, path: Option<&str>) -> Result<ResolvedPath, WorkspaceError> {
        let requested = path.unwrap_or(".");
        let resolved = self.resolve(requested)?;
        if !resolved.metadata.is_dir() {
            return Err(WorkspaceError::WrongKind {
                path: PathBuf::from(requested),
                expected: "directory",
                actual: "file",
            });
        }
        Ok(resolved)
    }

    pub(crate) fn resolve_file(&self, path: &str) -> Result<ResolvedPath, WorkspaceError> {
        let resolved = self.resolve(path)?;
        if !resolved.metadata.is_file() {
            return Err(WorkspaceError::WrongKind {
                path: PathBuf::from(path),
                expected: "file",
                actual: if resolved.metadata.is_dir() {
                    "directory"
                } else {
                    "non-regular file"
                },
            });
        }
        Ok(resolved)
    }

    /// Reads a resolved file as UTF-8. Taking `ResolvedPath` rather than
    /// `&Path` is the point: an unchecked path cannot be passed in at all.
    pub(crate) fn read_utf8_file(&self, file: &ResolvedPath) -> Result<String, WorkspaceError> {
        if file.metadata.len() > MAX_FILE_SIZE {
            return Err(WorkspaceError::TooLarge {
                path: file.path.clone(),
                max: MAX_FILE_SIZE,
            });
        }
        let bytes = fs::read(&file.path).map_err(|source| WorkspaceError::Io {
            path: file.path.clone(),
            source,
        })?;
        String::from_utf8(bytes).map_err(|_| WorkspaceError::NotUtf8(file.path.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn workspace(dir: &TempDir) -> Workspace {
        Workspace::new(dir.path().to_path_buf()).expect("workspace root should be valid")
    }

    #[test]
    fn new_rejects_a_file_as_root() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("not_a_dir");
        fs::write(&file, b"x").unwrap();

        let err = Workspace::new(file).unwrap_err();

        assert!(matches!(err, WorkspaceError::RootNotADirectory(_)));
    }

    #[test]
    fn new_reports_a_missing_root() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("missing");

        let err = Workspace::new(missing).unwrap_err();

        assert!(matches!(err, WorkspaceError::Io { .. }));
    }

    #[test]
    fn resolve_dir_defaults_to_root_when_path_omitted() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let resolved = ws.resolve_dir(None).unwrap();

        assert_eq!(resolved.as_path(), fs::canonicalize(dir.path()).unwrap());
    }

    #[test]
    fn resolve_dir_rejects_explicit_empty_path() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_dir(Some("")).unwrap_err();

        assert!(matches!(err, WorkspaceError::EmptyPath));
    }

    #[test]
    fn resolve_file_rejects_empty_path() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("").unwrap_err();

        assert!(matches!(err, WorkspaceError::EmptyPath));
    }

    #[test]
    fn resolve_rejects_absolute_paths() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("/etc/passwd").unwrap_err();

        assert!(matches!(err, WorkspaceError::AbsolutePath(_)));
    }

    #[test]
    fn resolve_rejects_parent_traversal_outside_root() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("../outside").unwrap_err();

        assert!(matches!(
            err,
            WorkspaceError::OutsideWorkspace(_) | WorkspaceError::NotFound(_)
        ));
    }

    #[test]
    fn resolve_allows_normalized_paths_that_stay_inside_root() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/file.txt"), b"hi").unwrap();
        let ws = workspace(&dir);

        let resolved = ws.resolve_file("sub/../sub/file.txt").unwrap();

        assert_eq!(
            resolved.as_path(),
            fs::canonicalize(dir.path().join("sub/file.txt")).unwrap()
        );
    }

    #[test]
    fn resolve_reports_missing_targets() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("nope.txt").unwrap_err();

        assert!(matches!(err, WorkspaceError::NotFound(_)));
    }

    /// I4 (docs/architecture.md): a model-visible error must report the
    /// workspace-relative path the model sent, not this machine's absolute
    /// path to it. See ADR-0005.
    #[test]
    fn not_found_reports_the_requested_path_not_the_absolute_host_path() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("sub/nope.txt").unwrap_err();

        let message = err.to_string();
        assert!(message.contains("sub/nope.txt"));
        assert!(!message.contains(dir.path().to_str().unwrap()));
    }

    #[test]
    fn resolve_dir_rejects_a_file_target() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"hi").unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_dir(Some("file.txt")).unwrap_err();

        assert!(matches!(err, WorkspaceError::WrongKind { .. }));
    }

    /// I4 (docs/architecture.md): see ADR-0005.
    #[test]
    fn wrong_kind_reports_the_requested_path_not_the_absolute_host_path() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"hi").unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_dir(Some("file.txt")).unwrap_err();

        let message = err.to_string();
        assert!(message.contains("file.txt"));
        assert!(!message.contains(dir.path().to_str().unwrap()));
    }

    #[test]
    fn resolve_file_rejects_a_directory_target() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_file("sub").unwrap_err();

        assert!(matches!(err, WorkspaceError::WrongKind { .. }));
    }

    #[test]
    fn read_utf8_file_rejects_oversized_files() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("big.txt");
        fs::write(&path, vec![b'a'; (MAX_FILE_SIZE + 1) as usize]).unwrap();
        let ws = workspace(&dir);

        let resolved = ws.resolve_file("big.txt").unwrap();
        let err = ws.read_utf8_file(&resolved).unwrap_err();

        assert!(matches!(err, WorkspaceError::TooLarge { .. }));
    }

    #[test]
    fn read_utf8_file_rejects_invalid_utf8() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("invalid.txt");
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        let ws = workspace(&dir);

        let resolved = ws.resolve_file("invalid.txt").unwrap();
        let err = ws.read_utf8_file(&resolved).unwrap_err();

        assert!(matches!(err, WorkspaceError::NotUtf8(_)));
    }

    #[test]
    fn read_utf8_file_returns_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hello.txt");
        fs::write(&path, "hello\nworld\n").unwrap();
        let ws = workspace(&dir);

        let resolved = ws.resolve_file("hello.txt").unwrap();
        let content = ws.read_utf8_file(&resolved).unwrap();

        assert_eq!(content, "hello\nworld\n");
    }

    #[cfg(unix)]
    #[test]
    fn resolve_rejects_a_symlink_to_outside_the_workspace() {
        use std::os::unix::fs::symlink;

        let workspace_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();

        let link = workspace_dir.path().join("escape");
        symlink(&outside_file, &link).unwrap();

        let ws = workspace(&workspace_dir);

        let err = ws.resolve_file("escape").unwrap_err();

        assert!(matches!(err, WorkspaceError::OutsideWorkspace(_)));
    }

    /// I4 (docs/architecture.md): see ADR-0005. The canonical escape target
    /// has no workspace-relative form to report, so the requested path (what
    /// the model sent) is what should show up instead.
    #[cfg(unix)]
    #[test]
    fn outside_workspace_reports_the_requested_path_not_the_absolute_host_path() {
        use std::os::unix::fs::symlink;

        let workspace_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("secret.txt");
        fs::write(&outside_file, b"secret").unwrap();

        let link = workspace_dir.path().join("escape");
        symlink(&outside_file, &link).unwrap();

        let ws = workspace(&workspace_dir);

        let err = ws.resolve_file("escape").unwrap_err();

        let message = err.to_string();
        assert!(message.contains("escape"));
        assert!(!message.contains(outside_dir.path().to_str().unwrap()));
    }
}
