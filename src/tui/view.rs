//! Rendering: a fixed-height live region (input + footer) drawn into the
//! inline viewport, and pure message wrapping for flushing finalized
//! messages into the terminal's native scrollback via `insert_before`.
//!
//! `unicode-width` operates on `char`s with no grapheme-cluster concept, so
//! combining marks and ZWJ emoji sequences will occasionally render a column
//! off; grapheme-correct wrapping is deliberately deferred.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::Widget;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::tui::app::{App, Message, Role, Status};

const PROMPT: &str = "› ";
const HELP_FOOTER: &str = "Enter send · Ctrl-C quit";
const THINKING_FOOTER: &str = "Thinking…";
const STOPPED_FOOTER: &str = "Agent stopped";

pub(crate) const LIVE_REGION_HEIGHT: u16 = 2;

/// Draws the live region: row 1 is the input prompt, row 2 is the footer.
pub(crate) fn render_live_region(frame: &mut Frame, area: Rect, app: &App) {
    let input_row = Rect {
        height: 1.min(area.height),
        ..area
    };
    if area.height > 1 {
        let footer_row = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
        frame.render_widget(Line::raw(footer_text(app.status())), footer_row);
    }

    let prompt_width = PROMPT.width() as u16;
    let available = input_row.width.saturating_sub(prompt_width);
    let visible_input = scroll_to_fit(app.input(), available);
    let visible_width = visible_input.width() as u16;
    frame.render_widget(Line::raw(format!("{PROMPT}{visible_input}")), input_row);

    if input_row.width > 0 {
        let cursor_x = prompt_width
            .saturating_add(visible_width)
            .min(input_row.width.saturating_sub(1));
        frame.set_cursor_position(Position::new(input_row.x + cursor_x, input_row.y));
    }
}

fn footer_text(status: Status) -> &'static str {
    match status {
        Status::Idle => HELP_FOOTER,
        Status::Thinking => THINKING_FOOTER,
        Status::Stopped => STOPPED_FOOTER,
    }
}

/// Returns the longest suffix of `input` (by whole chars) whose display width
/// fits within `available_width`, so the cursor at the end of the buffer
/// stays visible through horizontal scrolling.
pub(crate) fn scroll_to_fit(input: &str, available_width: u16) -> String {
    if available_width == 0 {
        return String::new();
    }

    let mut used: u16 = 0;
    let mut start = input.len();
    for (idx, ch) in input.char_indices().rev() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0) as u16;
        if used.saturating_add(w) > available_width {
            break;
        }
        used += w;
        start = idx;
    }
    input[start..].to_string()
}

/// Labels the message with its role and wraps it to `width` for
/// `Terminal::insert_before`. `result.len()` is exactly the `height` the
/// caller must pass to `insert_before`.
pub(crate) fn wrap_message(message: &Message, width: u16) -> Vec<Line<'static>> {
    let (label, color) = match message.role {
        Role::User => ("You: ", Color::Cyan),
        Role::Assistant => ("Fey: ", Color::Green),
        Role::Error => ("Error: ", Color::Red),
    };
    let style = Style::default().fg(color);
    let combined = format!("{label}{}", message.text);

    wrap_text(&combined, width)
        .into_iter()
        .map(|line| Line::styled(line, style))
        .collect()
}

/// Renders pre-wrapped lines into a buffer, for use inside an
/// `insert_before` draw closure.
pub(crate) fn render_message(lines: Vec<Line<'static>>, buf: &mut Buffer) {
    Text::from(lines).render(buf.area, buf);
}

fn wrap_text(text: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let width = width as usize;

    let mut lines = Vec::new();
    let mut line = String::new();
    let mut line_width = 0usize;
    let mut word = String::new();
    let mut word_width = 0usize;

    let flush_word = |line: &mut String,
                      line_width: &mut usize,
                      lines: &mut Vec<String>,
                      word: &mut String,
                      word_width: &mut usize| {
        if word.is_empty() {
            return;
        }
        if *line_width > 0 && *line_width + *word_width > width {
            lines.push(std::mem::take(line));
            *line_width = 0;
        }
        for c in word.chars() {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if *line_width > 0 && *line_width + w > width {
                lines.push(std::mem::take(line));
                *line_width = 0;
            }
            line.push(c);
            *line_width += w;
        }
        word.clear();
        *word_width = 0;
    };

    for c in text.chars() {
        if c.is_whitespace() {
            flush_word(
                &mut line,
                &mut line_width,
                &mut lines,
                &mut word,
                &mut word_width,
            );
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            if w == 0 {
                continue;
            }
            if line_width + w > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            } else {
                line.push(c);
                line_width += w;
            }
        } else {
            let w = UnicodeWidthChar::width(c).unwrap_or(0);
            word.push(c);
            word_width += w;
        }
    }
    flush_word(
        &mut line,
        &mut line_width,
        &mut lines,
        &mut word,
        &mut word_width,
    );
    lines.push(line);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::Role;

    fn message(role: Role, text: &str) -> Message {
        Message {
            role,
            text: text.to_string(),
        }
    }

    fn plain_lines(message: &Message, width: u16) -> Vec<String> {
        wrap_message(message, width)
            .into_iter()
            .map(|line| line.to_string())
            .collect()
    }

    #[test]
    fn wraps_a_short_message_onto_one_labeled_line() {
        let lines = plain_lines(&message(Role::User, "hi"), 80);

        assert_eq!(lines, vec!["You: hi".to_string()]);
    }

    #[test]
    fn wraps_an_empty_message_onto_one_labeled_line() {
        let lines = plain_lines(&message(Role::Assistant, ""), 80);

        assert_eq!(lines, vec!["Fey: ".to_string()]);
    }

    #[test]
    fn breaks_on_whitespace_at_the_available_width() {
        let lines = plain_lines(&message(Role::Assistant, "aaa bbb ccc"), 8);

        assert_eq!(lines, vec!["Fey: aaa".to_string(), "bbb ccc".to_string()]);
    }

    #[test]
    fn hard_breaks_a_single_word_wider_than_the_width() {
        let lines = plain_lines(&message(Role::Error, "abcdefghijk"), 10);

        assert_eq!(
            lines,
            vec![
                "Error: ".to_string(),
                "abcdefghij".to_string(),
                "k".to_string(),
            ]
        );
    }

    #[test]
    fn accounts_for_wide_characters() {
        let lines = plain_lines(&message(Role::Assistant, "\u{4f60}\u{597d}"), 9);

        assert_eq!(lines, vec!["Fey: \u{4f60}\u{597d}".to_string()]);
    }

    #[test]
    fn hard_breaks_wide_characters_that_do_not_fit_together() {
        let lines = plain_lines(&message(Role::Assistant, "\u{4f60}\u{597d}\u{4e16}"), 5);

        assert_eq!(
            lines,
            vec![
                "Fey: ".to_string(),
                "\u{4f60}\u{597d}".to_string(),
                "\u{4e16}".to_string(),
            ]
        );
    }

    #[test]
    fn wrap_message_len_matches_the_line_count() {
        let lines = wrap_message(&message(Role::User, "aaa bbb ccc"), 8);

        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn scroll_to_fit_returns_empty_string_for_zero_width() {
        assert_eq!(scroll_to_fit("hello", 0), "");
    }

    #[test]
    fn scroll_to_fit_keeps_the_tail_when_input_overflows() {
        assert_eq!(scroll_to_fit("hello world", 5), "world");
    }

    #[test]
    fn scroll_to_fit_keeps_short_input_unchanged() {
        assert_eq!(scroll_to_fit("hi", 10), "hi");
    }
}
