//! Filesystem behavior for the `list_directory` tool, kept separate from Rig adapter code.

use std::fs;

use crate::workspace::{Workspace, WorkspaceError};

const MAX_ENTRIES: usize = 200;

enum EntryKind {
    Directory,
    Symlink,
    Other,
}

pub(crate) fn list_directory(
    workspace: &Workspace,
    path: Option<&str>,
) -> Result<String, WorkspaceError> {
    let dir = workspace.resolve_dir(path)?;

    let mut entries: Vec<(String, EntryKind)> = fs::read_dir(&dir)
        .map_err(|source| WorkspaceError::Io {
            path: dir.clone(),
            source,
        })?
        .map(|entry| {
            let entry = entry.map_err(|source| WorkspaceError::Io {
                path: dir.clone(),
                source,
            })?;
            let file_type = entry.file_type().map_err(|source| WorkspaceError::Io {
                path: entry.path(),
                source,
            })?;
            let kind = if file_type.is_symlink() {
                EntryKind::Symlink
            } else if file_type.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::Other
            };
            Ok((entry.file_name().to_string_lossy().into_owned(), kind))
        })
        .collect::<Result<Vec<_>, WorkspaceError>>()?;

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    if entries.is_empty() {
        return Ok("(empty directory)".to_string());
    }

    let truncated = entries.len() > MAX_ENTRIES;
    let mut lines: Vec<String> = entries
        .iter()
        .take(MAX_ENTRIES)
        .map(|(name, kind)| match kind {
            EntryKind::Directory => format!("{name}/"),
            EntryKind::Symlink => format!("{name}@"),
            EntryKind::Other => name.clone(),
        })
        .collect();

    if truncated {
        lines.push("... additional entries omitted".to_string());
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn workspace(dir: &TempDir) -> Workspace {
        Workspace::new(dir.path().to_path_buf()).expect("workspace root should be valid")
    }

    #[test]
    fn lists_entries_sorted_with_type_suffixes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("b.txt"), b"b").unwrap();
        fs::create_dir(dir.path().join("a_dir")).unwrap();
        fs::write(dir.path().join("c.txt"), b"c").unwrap();
        let ws = workspace(&dir);

        let output = list_directory(&ws, None).unwrap();

        assert_eq!(output, "a_dir/\nb.txt\nc.txt");
    }

    #[test]
    fn reports_an_empty_directory() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let output = list_directory(&ws, None).unwrap();

        assert_eq!(output, "(empty directory)");
    }

    #[test]
    fn truncates_beyond_two_hundred_entries() {
        let dir = TempDir::new().unwrap();
        for i in 0..201 {
            fs::write(dir.path().join(format!("file_{i:04}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let output = list_directory(&ws, None).unwrap();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), MAX_ENTRIES + 1);
        assert_eq!(lines[MAX_ENTRIES], "... additional entries omitted");
    }

    #[test]
    fn does_not_truncate_at_exactly_two_hundred_entries() {
        let dir = TempDir::new().unwrap();
        for i in 0..200 {
            fs::write(dir.path().join(format!("file_{i:04}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let output = list_directory(&ws, None).unwrap();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), MAX_ENTRIES);
        assert!(!output.contains("omitted"));
    }

    #[test]
    fn rejects_a_missing_directory() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("missing")).unwrap_err();

        assert!(matches!(err, WorkspaceError::NotFound(_)));
    }

    #[test]
    fn rejects_a_file_target() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"x").unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("file.txt")).unwrap_err();

        assert!(matches!(err, WorkspaceError::WrongKind { .. }));
    }

    #[test]
    fn rejects_escaping_the_workspace() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("../")).unwrap_err();

        assert!(matches!(
            err,
            WorkspaceError::OutsideWorkspace(_) | WorkspaceError::NotFound(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn suffixes_symlinks_with_at_sign() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("target.txt"), b"x").unwrap();
        symlink(dir.path().join("target.txt"), dir.path().join("link")).unwrap();
        let ws = workspace(&dir);

        let output = list_directory(&ws, None).unwrap();

        assert_eq!(output, "link@\ntarget.txt");
    }
}
