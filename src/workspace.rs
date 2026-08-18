//! Filesystem sandbox: confines tool access to a canonical workspace root.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub(crate) const MAX_FILE_SIZE: u64 = 1024 * 1024;

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

    fn resolve(&self, path: &str) -> Result<PathBuf, WorkspaceError> {
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
                WorkspaceError::NotFound(candidate.clone())
            } else {
                WorkspaceError::Io {
                    path: candidate.clone(),
                    source,
                }
            }
        })?;

        if !canonical.starts_with(&self.root) {
            return Err(WorkspaceError::OutsideWorkspace(canonical));
        }

        Ok(canonical)
    }

    pub(crate) fn resolve_dir(&self, path: Option<&str>) -> Result<PathBuf, WorkspaceError> {
        let resolved = self.resolve(path.unwrap_or("."))?;
        if !resolved.is_dir() {
            return Err(WorkspaceError::WrongKind {
                path: resolved,
                expected: "directory",
                actual: "file",
            });
        }
        Ok(resolved)
    }

    pub(crate) fn resolve_file(&self, path: &str) -> Result<PathBuf, WorkspaceError> {
        let resolved = self.resolve(path)?;
        let metadata = fs::metadata(&resolved).map_err(|source| WorkspaceError::Io {
            path: resolved.clone(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(WorkspaceError::WrongKind {
                path: resolved,
                expected: "file",
                actual: if metadata.is_dir() {
                    "directory"
                } else {
                    "non-regular file"
                },
            });
        }
        Ok(resolved)
    }

    pub(crate) fn read_utf8_file(&self, path: &Path) -> Result<String, WorkspaceError> {
        let metadata = fs::metadata(path).map_err(|source| WorkspaceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(WorkspaceError::TooLarge {
                path: path.to_path_buf(),
                max: MAX_FILE_SIZE,
            });
        }
        let bytes = fs::read(path).map_err(|source| WorkspaceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        String::from_utf8(bytes).map_err(|_| WorkspaceError::NotUtf8(path.to_path_buf()))
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

        assert_eq!(resolved, fs::canonicalize(dir.path()).unwrap());
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
            resolved,
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

    #[test]
    fn resolve_dir_rejects_a_file_target() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"hi").unwrap();
        let ws = workspace(&dir);

        let err = ws.resolve_dir(Some("file.txt")).unwrap_err();

        assert!(matches!(err, WorkspaceError::WrongKind { .. }));
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

        let err = ws
            .read_utf8_file(&fs::canonicalize(&path).unwrap())
            .unwrap_err();

        assert!(matches!(err, WorkspaceError::TooLarge { .. }));
    }

    #[test]
    fn read_utf8_file_rejects_invalid_utf8() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("invalid.txt");
        fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        let ws = workspace(&dir);

        let err = ws
            .read_utf8_file(&fs::canonicalize(&path).unwrap())
            .unwrap_err();

        assert!(matches!(err, WorkspaceError::NotUtf8(_)));
    }

    #[test]
    fn read_utf8_file_returns_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("hello.txt");
        fs::write(&path, "hello\nworld\n").unwrap();
        let ws = workspace(&dir);

        let content = ws
            .read_utf8_file(&fs::canonicalize(&path).unwrap())
            .unwrap();

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
}
