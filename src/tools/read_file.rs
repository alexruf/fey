//! Filesystem behavior for the `read_file` tool, kept separate from Rig adapter code.

use thiserror::Error;

use crate::workspace::{Workspace, WorkspaceError};

const MAX_LINES: usize = 200;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Why the output was cut short. The distinction matters to the model: one
/// case is recoverable by asking again, the other is not.
enum Truncation {
    /// Whole lines were left out. A later `start_line` reaches them.
    MoreLines,
    /// One line was too large to fit even on its own, so it was clipped.
    /// No `start_line` can ever reach the remainder of that line.
    ClippedLine(usize),
}

#[derive(Debug, Error)]
pub(crate) enum ReadFileError {
    #[error("start_line must be at least 1")]
    ZeroStartLine,
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
}

/// Truncates `text` to at most `max` bytes without splitting a character.
///
/// Slicing a `&str` at an arbitrary byte index panics when it lands inside a
/// multi-byte character, so back off to the nearest boundary first.
fn clip_to_bytes(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
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
    let mut truncation: Option<Truncation> = None;

    for (offset, line) in lines[start_index..].iter().enumerate() {
        if formatted_lines.len() >= MAX_LINES {
            truncation = Some(Truncation::MoreLines);
            break;
        }
        let line_number = start_line + offset;
        let formatted = format!("{line_number}: {line}");
        let separator = usize::from(!formatted_lines.is_empty());
        if byte_count + separator + formatted.len() > MAX_OUTPUT_BYTES {
            if formatted_lines.is_empty() {
                // This line does not fit even with the whole budget to
                // itself, so skipping it would hide it permanently: every
                // retry starts here again and returns nothing. Hand back a
                // clipped prefix instead of an empty result.
                formatted_lines.push(clip_to_bytes(&formatted, MAX_OUTPUT_BYTES));
                truncation = Some(Truncation::ClippedLine(line_number));
            } else {
                truncation = Some(Truncation::MoreLines);
            }
            break;
        }
        byte_count += separator + formatted.len();
        formatted_lines.push(formatted);
    }

    let mut output = formatted_lines.join("\n");
    if let Some(truncation) = truncation {
        if !output.is_empty() {
            output.push('\n');
        }
        match truncation {
            Truncation::MoreLines => output
                .push_str("... output truncated; call read_file again with a later start_line"),
            Truncation::ClippedLine(line_number) => output.push_str(&format!(
                "... line {line_number} exceeds {MAX_OUTPUT_BYTES} bytes and was clipped; \
                 the rest of that line is unreachable, continue at start_line {}",
                line_number + 1
            )),
        }
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

    #[test]
    fn clips_a_line_too_large_to_fit_instead_of_returning_nothing() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("min.js"), "x".repeat(70 * 1024)).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "min.js", None).unwrap();

        assert!(output.starts_with("1: xxx"), "content must be returned");
        assert!(output.len() > MAX_OUTPUT_BYTES / 2);
        assert!(output.contains("was clipped"));
    }

    /// The old advice ("call again with a later start_line") was unreachable
    /// for a single over-long line: the retry landed on the same line and
    /// returned nothing, so the model could loop forever.
    #[test]
    fn a_clipped_line_points_at_the_next_reachable_line() {
        let dir = TempDir::new().unwrap();
        let content = format!("{}\nsecond line\n", "x".repeat(70 * 1024));
        fs::write(dir.path().join("min.js"), content).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "min.js", None).unwrap();
        assert!(output.contains("continue at start_line 2"));

        let follow_up = read_file(&ws, "min.js", Some(2)).unwrap();
        assert_eq!(follow_up, "2: second line");
    }

    #[test]
    fn clipping_never_splits_a_multi_byte_character() {
        let dir = TempDir::new().unwrap();
        // 2 bytes per char, so the clip boundary lands mid-character unless
        // it is moved back to a char boundary.
        fs::write(dir.path().join("wide.txt"), "ä".repeat(40 * 1024)).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "wide.txt", None).unwrap();

        assert!(output.contains("was clipped"));
        assert!(output.starts_with("1: ää"));
    }

    #[test]
    fn whole_lines_left_out_still_advise_a_later_start_line() {
        let dir = TempDir::new().unwrap();
        let content: String = (1..=201).map(|i| format!("line{i}\n")).collect();
        fs::write(dir.path().join("f.txt"), content).unwrap();
        let ws = workspace(&dir);

        let output = read_file(&ws, "f.txt", None).unwrap();

        assert!(output.ends_with("call read_file again with a later start_line"));
        assert!(!output.contains("was clipped"));
    }
}
