//! Pure TUI state: the input buffer, current status, and the actions that
//! key handling returns. Owns no transcript — finalized messages are written
//! once into the terminal's native scrollback and never redrawn, so nothing
//! about their history needs to live here.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use fey::{AgentError, AgentReply};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    User,
    Assistant,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Message {
    pub(crate) role: Role,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Idle,
    Thinking,
    Stopped,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Edit,
    Submit(Message),
    Quit,
    None,
}

/// Worker->UI channel payload. Deliberately not `Result<AgentReply, AgentError>`:
/// tool-call visibility is the intended next feature, and adding a `ToolCall`
/// variant later must not be a signature change at every call site.
pub(crate) enum WorkerEvent {
    Reply(AgentReply),
    Failed(AgentError),
}

pub(crate) struct App {
    input: String,
    status: Status,
}

impl App {
    pub(crate) fn new() -> Self {
        Self {
            input: String::new(),
            status: Status::Idle,
        }
    }

    pub(crate) fn input(&self) -> &str {
        &self.input
    }

    pub(crate) fn status(&self) -> Status {
        self.status
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return Action::Quit;
        }

        match key.code {
            KeyCode::Enter => self.submit(),
            // Only Shift may accompany a printable character. Without this
            // guard, Ctrl-A or Alt-B arrive as `Char('a')`/`Char('b')` and get
            // typed into the buffer as literal text.
            KeyCode::Char(c)
                if self.status == Status::Idle && is_plain_or_shifted(key.modifiers) =>
            {
                self.input.push(c);
                Action::Edit
            }
            KeyCode::Backspace if self.status == Status::Idle => {
                if self.input.pop().is_some() {
                    Action::Edit
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    fn submit(&mut self) -> Action {
        if self.status != Status::Idle {
            return Action::None;
        }
        let trimmed = self.input.trim();
        if trimmed.is_empty() {
            return Action::None;
        }

        let message = Message {
            role: Role::User,
            text: trimmed.to_string(),
        };
        self.input.clear();
        self.status = Status::Thinking;
        Action::Submit(message)
    }

    /// Called once a worker event arrives: restores `Idle` and returns the
    /// message to flush.
    pub(crate) fn complete(&mut self, event: WorkerEvent) -> Message {
        self.status = Status::Idle;
        match event {
            WorkerEvent::Reply(reply) => Message {
                role: Role::Assistant,
                text: reply.text,
            },
            WorkerEvent::Failed(err) => Message {
                role: Role::Error,
                text: error_chain(&err),
            },
        }
    }

    /// One-shot transition to `Stopped` for a disconnected worker channel.
    /// Returns `None` on repeated calls so the error is flushed only once.
    pub(crate) fn stop(&mut self) -> Option<Message> {
        if self.status == Status::Stopped {
            return None;
        }
        self.status = Status::Stopped;
        Some(Message {
            role: Role::Error,
            text: "agent worker stopped".to_string(),
        })
    }
}

/// Whether these modifiers describe a character the user meant to type.
///
/// Anything beyond Shift (Ctrl, Alt, Super, …) is a command, not text.
fn is_plain_or_shifted(modifiers: KeyModifiers) -> bool {
    modifiers.difference(KeyModifiers::SHIFT).is_empty()
}

/// Renders an error together with every cause behind it.
///
/// `Display` on a `thiserror` enum only describes its own layer — the useful
/// detail (a refused connection, a missing model) hangs off `source()`. Without
/// walking the chain, an unreachable Ollama server reaches the user as nothing
/// but "agent turn failed".
///
/// Not every error type follows that convention though. Rig's do interpolate
/// their own source into their `Display`, so appending each cause blindly
/// repeats the same sentence — in the unreachable-server case, the same URL
/// five times over. A cause is only added when it says something the message
/// does not already contain.
fn error_chain(err: &dyn std::error::Error) -> String {
    let mut rendered = err.to_string();
    let mut cause = err.source();
    while let Some(current) = cause {
        let message = current.to_string();
        if !rendered.contains(&message) {
            rendered.push_str(": ");
            rendered.push_str(&message);
        }
        cause = current.source();
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }

    #[test]
    fn enter_ignores_whitespace_only_input() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char(' ')));

        let action = app.handle_key(key(KeyCode::Enter));

        assert_eq!(action, Action::None);
        assert_eq!(app.status(), Status::Idle);
    }

    #[test]
    fn enter_while_idle_submits_trimmed_input_and_starts_thinking() {
        let mut app = App::new();
        for c in " hi ".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }

        let action = app.handle_key(key(KeyCode::Enter));

        assert_eq!(
            action,
            Action::Submit(Message {
                role: Role::User,
                text: "hi".to_string(),
            })
        );
        assert_eq!(app.status(), Status::Thinking);
        assert_eq!(app.input(), "");
    }

    #[test]
    fn enter_while_thinking_is_ignored() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Enter));

        let action = app.handle_key(key(KeyCode::Enter));

        assert_eq!(action, Action::None);
    }

    #[test]
    fn enter_while_stopped_is_ignored() {
        let mut app = App::new();
        app.stop();

        let action = app.handle_key(key(KeyCode::Enter));

        assert_eq!(action, Action::None);
    }

    #[test]
    fn ctrl_c_emits_quit_while_idle() {
        let mut app = App::new();

        assert_eq!(app.handle_key(ctrl_c()), Action::Quit);
    }

    #[test]
    fn ctrl_c_emits_quit_while_thinking() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.handle_key(ctrl_c()), Action::Quit);
    }

    #[test]
    fn ctrl_c_emits_quit_while_stopped() {
        let mut app = App::new();
        app.stop();

        assert_eq!(app.handle_key(ctrl_c()), Action::Quit);
    }

    #[test]
    fn character_input_while_thinking_is_ignored() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Enter));

        let action = app.handle_key(key(KeyCode::Char('b')));

        assert_eq!(action, Action::None);
        assert_eq!(app.input(), "");
    }

    #[test]
    fn backspace_on_empty_input_is_ignored() {
        let mut app = App::new();

        assert_eq!(app.handle_key(key(KeyCode::Backspace)), Action::None);
    }

    #[test]
    fn complete_restores_idle_and_returns_assistant_message_on_reply() {
        let mut app = App::new();
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Enter));

        let message = app.complete(WorkerEvent::Reply(AgentReply {
            text: "hi there".to_string(),
        }));

        assert_eq!(app.status(), Status::Idle);
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.text, "hi there");
    }

    #[test]
    fn stop_transitions_once_and_returns_none_afterwards() {
        let mut app = App::new();

        let first = app.stop();
        let second = app.stop();

        assert_eq!(app.status(), Status::Stopped);
        assert!(first.is_some());
        assert_eq!(first.unwrap().role, Role::Error);
        assert!(second.is_none());
    }

    #[test]
    fn complete_reports_every_cause_behind_a_failure() {
        let mut app = App::new();
        let event = WorkerEvent::Failed(AgentError::Workspace {
            path: std::path::PathBuf::from("/tmp/ws"),
            source: Box::new(std::io::Error::other("connection refused")),
        });

        let message = app.complete(event);

        assert_eq!(message.role, Role::Error);
        assert_eq!(
            message.text,
            "failed to initialize workspace at \"/tmp/ws\": connection refused"
        );
    }

    #[test]
    fn control_and_alt_combinations_never_reach_the_input_buffer() {
        let mut app = App::new();

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT));
        app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::SUPER));

        assert_eq!(app.input(), "");
    }

    #[test]
    fn shifted_characters_still_reach_the_input_buffer() {
        let mut app = App::new();

        app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::SHIFT));
        app.handle_key(key(KeyCode::Char('i')));

        assert_eq!(app.input(), "Hi");
    }

    #[test]
    fn a_rejected_combination_reports_no_edit() {
        let mut app = App::new();

        let action = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

        assert_eq!(action, Action::None);
    }

    /// Mirrors how Rig's error types behave: each layer prints its own source
    /// as part of its own message.
    #[derive(Debug)]
    struct Layer {
        message: String,
        cause: Option<Box<Layer>>,
    }

    impl Layer {
        fn new(message: &str, cause: Option<Layer>) -> Self {
            Self {
                message: message.to_string(),
                cause: cause.map(Box::new),
            }
        }
    }

    impl std::fmt::Display for Layer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.message)
        }
    }

    impl std::error::Error for Layer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.cause
                .as_deref()
                .map(|cause| cause as &(dyn std::error::Error + 'static))
        }
    }

    #[test]
    fn error_chain_skips_a_cause_already_spelled_out_by_its_parent() {
        let inner = Layer::new("connection refused", None);
        let outer = Layer::new("request failed: connection refused", Some(inner));

        assert_eq!(error_chain(&outer), "request failed: connection refused");
    }

    #[test]
    fn error_chain_keeps_a_cause_that_adds_detail() {
        let root = Layer::new("os error 61", None);
        let middle = Layer::new("connection refused", Some(root));
        let outer = Layer::new("request failed: connection refused", Some(middle));

        assert_eq!(
            error_chain(&outer),
            "request failed: connection refused: os error 61"
        );
    }
}
