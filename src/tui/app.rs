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
            KeyCode::Char(c) if self.status == Status::Idle => {
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
                text: err.to_string(),
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
}
