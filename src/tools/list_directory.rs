//! Filesystem behavior for the `list_directory` tool, kept separate from Rig adapter code.

use std::fs;

use thiserror::Error;

use crate::workspace::{Workspace, WorkspaceError};

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Error)]
pub(crate) enum ListDirectoryError {
    #[error("start_entry must be at least 1")]
    ZeroStartEntry,
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

enum EntryKind {
    Directory,
    Symlink,
    Other,
}

pub(crate) fn list_directory(
    workspace: &Workspace,
    path: Option<&str>,
    start_entry: Option<usize>,
) -> Result<String, ListDirectoryError> {
    let start_entry = start_entry.unwrap_or(1);
    if start_entry == 0 {
        return Err(ListDirectoryError::ZeroStartEntry);
    }

    let dir = workspace.resolve_dir(path)?;

    let mut entries: Vec<(String, EntryKind)> = fs::read_dir(dir.as_path())
        .map_err(|source| WorkspaceError::Io {
            path: dir.as_path().to_path_buf(),
            source,
        })?
        .map(|entry| {
            let entry = entry.map_err(|source| WorkspaceError::Io {
                path: dir.as_path().to_path_buf(),
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

    // Names within one directory are unique, so a stable sort buys nothing.
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    if entries.is_empty() {
        return Ok("(empty directory)".to_string());
    }

    let total = entries.len();
    let start_index = start_entry - 1;
    if start_index >= total {
        return Ok(format!("(no entries at or after entry {start_entry})"));
    }

    let mut lines: Vec<String> = entries[start_index..]
        .iter()
        .take(MAX_ENTRIES)
        .map(|(name, kind)| match kind {
            EntryKind::Directory => format!("{name}/"),
            EntryKind::Symlink => format!("{name}@"),
            EntryKind::Other => name.clone(),
        })
        .collect();

    // Say how to reach the rest. A bare "entries omitted" would tell the model
    // that something is missing while leaving it no way to ask for it.
    let listed_through = start_index + lines.len();
    if listed_through < total {
        let remaining = total - listed_through;
        let noun = if remaining == 1 { "entry" } else { "entries" };
        lines.push(format!(
            "... {remaining} more {noun}; call list_directory again with start_entry {}",
            listed_through + 1
        ));
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

        let output = list_directory(&ws, None, None).unwrap();

        assert_eq!(output, "a_dir/\nb.txt\nc.txt");
    }

    #[test]
    fn reports_an_empty_directory() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let output = list_directory(&ws, None, None).unwrap();

        assert_eq!(output, "(empty directory)");
    }

    #[test]
    fn truncates_beyond_two_hundred_entries() {
        let dir = TempDir::new().unwrap();
        for i in 0..201 {
            fs::write(dir.path().join(format!("file_{i:04}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let output = list_directory(&ws, None, None).unwrap();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), MAX_ENTRIES + 1);
        assert_eq!(
            lines[MAX_ENTRIES],
            "... 1 more entry; call list_directory again with start_entry 201"
        );
    }

    #[test]
    fn does_not_truncate_at_exactly_two_hundred_entries() {
        let dir = TempDir::new().unwrap();
        for i in 0..200 {
            fs::write(dir.path().join(format!("file_{i:04}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let output = list_directory(&ws, None, None).unwrap();
        let lines: Vec<&str> = output.lines().collect();

        assert_eq!(lines.len(), MAX_ENTRIES);
        assert!(!output.contains("more entries"));
    }

    #[test]
    fn rejects_a_missing_directory() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("missing"), None).unwrap_err();

        assert!(matches!(
            err,
            ListDirectoryError::Workspace(WorkspaceError::NotFound(_))
        ));
    }

    #[test]
    fn rejects_a_file_target() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), b"x").unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("file.txt"), None).unwrap_err();

        assert!(matches!(
            err,
            ListDirectoryError::Workspace(WorkspaceError::WrongKind { .. })
        ));
    }

    #[test]
    fn rejects_escaping_the_workspace() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, Some("../"), None).unwrap_err();

        assert!(matches!(
            err,
            ListDirectoryError::Workspace(
                WorkspaceError::OutsideWorkspace(_) | WorkspaceError::NotFound(_)
            )
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

        let output = list_directory(&ws, None, None).unwrap();

        assert_eq!(output, "link@\ntarget.txt");
    }

    #[test]
    fn pages_through_a_directory_from_a_later_start_entry() {
        let dir = TempDir::new().unwrap();
        for i in 0..3 {
            fs::write(dir.path().join(format!("file_{i}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let output = list_directory(&ws, None, Some(2)).unwrap();

        assert_eq!(output, "file_1.txt\nfile_2.txt");
    }

    /// The truncation notice has to be actionable: following it must reach the
    /// entries the first call left out.
    #[test]
    fn following_the_truncation_notice_reaches_the_remaining_entries() {
        let dir = TempDir::new().unwrap();
        for i in 0..201 {
            fs::write(dir.path().join(format!("file_{i:04}.txt")), b"x").unwrap();
        }
        let ws = workspace(&dir);

        let second_page = list_directory(&ws, None, Some(MAX_ENTRIES + 1)).unwrap();

        assert_eq!(second_page, "file_0200.txt");
    }

    #[test]
    fn reports_a_start_entry_beyond_the_last_entry() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("only.txt"), b"x").unwrap();
        let ws = workspace(&dir);

        let output = list_directory(&ws, None, Some(5)).unwrap();

        assert_eq!(output, "(no entries at or after entry 5)");
    }

    #[test]
    fn rejects_a_zero_start_entry() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = list_directory(&ws, None, Some(0)).unwrap_err();

        assert!(matches!(err, ListDirectoryError::ZeroStartEntry));
    }
}
