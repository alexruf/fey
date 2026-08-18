//! Filesystem behavior for the `read_file` tool, kept separate from Rig adapter code.

use thiserror::Error;

use crate::workspace::{Workspace, WorkspaceError};

const MAX_LINES: usize = 200;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub(crate) enum ReadFileError {
    #[error("start_line must be at least 1")]
    ZeroStartLine,
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

pub(crate) fn read_file(
    workspace: &Workspace,
    path: &str,
    start_line: Option<usize>,
) -> Result<String, ReadFileError> {
    let start_line = start_line.unwrap_or(1);
    if start_line == 0 {
        return Err(ReadFileError::ZeroStartLine);
    }

    let resolved = workspace.resolve_file(path)?;
    let content = workspace.read_utf8_file(&resolved)?;

    if content.is_empty() {
        return Ok("(empty file)".to_string());
    }

    let lines: Vec<&str> = content.lines().collect();
    let start_index = start_line - 1;
    if start_index >= lines.len() {
        return Ok(format!("(no content at or after line {start_line})"));
    }

    let mut formatted_lines: Vec<String> = Vec::new();
    let mut byte_count = 0usize;
    let mut truncated = false;

    for (offset, line) in lines[start_index..].iter().enumerate() {
        if formatted_lines.len() >= MAX_LINES {
            truncated = true;
            break;
        }
        let line_number = start_line + offset;
        let formatted = format!("{line_number}: {line}");
        let separator = usize::from(!formatted_lines.is_empty());
        if byte_count + separator + formatted.len() > MAX_OUTPUT_BYTES {
            truncated = true;
            break;
        }
        byte_count += separator + formatted.len();
        formatted_lines.push(formatted);
    }

    let mut output = formatted_lines.join("\n");
    if truncated {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("... output truncated; call read_file again with a later start_line");
    }

    Ok(output)
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
    fn reads_from_the_default_start_line() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "one\ntwo\nthree").unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();

        assert_eq!(output, "1: one\n2: two\n3: three");
    }

    #[test]
    fn reads_from_a_later_start_line() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "one\ntwo\nthree").unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", Some(2)).unwrap();

        assert_eq!(output, "2: two\n3: three");
    }

    #[test]
    fn reports_an_empty_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "").unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();

        assert_eq!(output, "(empty file)");
    }

    #[test]
    fn reports_start_line_beyond_eof() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "one\ntwo").unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", Some(10)).unwrap();

        assert_eq!(output, "(no content at or after line 10)");
    }

    #[test]
    fn rejects_a_zero_start_line() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "one").unwrap();
        let ws = workspace(&dir);

        let err = read_file(&ws, "f.txt", Some(0)).unwrap_err();

        assert!(matches!(err, ReadFileError::ZeroStartLine));
    }

    #[test]
    fn truncates_beyond_two_hundred_lines() {
        let dir = TempDir::new().unwrap();
        let content: String = (1..=201).map(|i| format!("line{i}\n")).collect();
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();

        assert!(output.contains("line200"));
        assert!(!output.contains("line201"));
        assert!(
            output.ends_with("... output truncated; call read_file again with a later start_line")
        );
    }

    #[test]
    fn does_not_truncate_at_exactly_two_hundred_lines() {
        let dir = TempDir::new().unwrap();
        let content: String = (1..=200).map(|i| format!("line{i}\n")).collect();
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();

        assert!(output.contains("line200"));
        assert!(!output.contains("truncated"));
    }

    #[test]
    fn truncates_beyond_the_byte_budget_without_splitting_a_line() {
        let dir = TempDir::new().unwrap();
        let long_line = "a".repeat(2000);
        let content: String = (0..64).map(|_| format!("{long_line}\n")).collect();
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();
        let byte_budget_line = output
            .lines()
            .find(|l| l.contains("truncated"))
            .expect("truncation message present");

        assert!(output.len() <= MAX_OUTPUT_BYTES + byte_budget_line.len() + 1);
        assert!(output.contains("truncated"));
    }

    #[test]
    fn rejects_oversized_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("big.txt"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
        let ws = workspace(&dir);

        let err = read_file(&ws, "big.txt", None).unwrap_err();

        assert!(matches!(
            err,
            ReadFileError::Workspace(WorkspaceError::TooLarge { .. })
        ));
    }

    #[test]
    fn rejects_invalid_utf8() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), [0xff, 0xfe]).unwrap();
        let ws = workspace(&dir);

        let err = read_file(&ws, "f.txt", None).unwrap_err();

        assert!(matches!(
            err,
            ReadFileError::Workspace(WorkspaceError::NotUtf8(_))
        ));
    }

    #[test]
    fn rejects_a_missing_file() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);

        let err = read_file(&ws, "missing.txt", None).unwrap_err();

        assert!(matches!(
            err,
            ReadFileError::Workspace(WorkspaceError::NotFound(_))
        ));
    }

    #[test]
    fn rejects_a_directory_target() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let ws = workspace(&dir);

        let err = read_file(&ws, "sub", None).unwrap_err();

        assert!(matches!(
            err,
            ReadFileError::Workspace(WorkspaceError::WrongKind { .. })
        ));
    }
}
