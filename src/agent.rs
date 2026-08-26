//! Ollama-backed agent session: wires the read-only tools to a Rig agent
//! behind a small, injectable-model API.

use std::path::PathBuf;

use rig::client::CompletionClient as _;
use rig::completion::message::UserContent;
use rig::completion::{CompletionModel, Message, Prompt, PromptError};
use rig::providers::ollama;
use rig::tool::ToolContext;
use thiserror::Error;

use crate::tools::{ListDirectory, ReadFile};
use crate::workspace::Workspace;

const CONVERSATION_ID: &str = "main";

/// How many past turns the model still sees. Tool results are part of a turn,
/// and a single `read_file` result can be tens of kilobytes, so the cap is on
/// turns rather than on messages. See
/// docs/decisions/0007-bounded-conversation-history.md.
const HISTORY_TURNS: usize = 8;
const PREAMBLE: &str = "You are Fey, a read-only coding assistant. Inspect the workspace with \
the available tools before answering questions about it. Never claim to have modified files or \
run commands; you cannot do either. Keep answers concise and cite workspace-relative paths when \
referring to code.";

pub struct AgentConfig {
    pub model: String,
    pub ollama_base_url: String,
    pub workspace_root: PathBuf,
}

pub struct AgentReply {
    pub text: String,
}

pub struct AgentSession {
    agent: rig::agent::Agent,
    workspace: Workspace,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("model name must not be empty")]
    EmptyModel,

    #[error("prompt must not be empty")]
    EmptyPrompt,

    // No `{source}` here: every consumer renders the whole chain (the TUI via
    // `error_chain`, `main` via anyhow's `Debug`), so interpolating the source
    // into this message would print it twice.
    #[error("failed to initialize workspace at {path:?}")]
    Workspace {
        path: PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("failed to construct the Ollama client")]
    OllamaClient(#[from] rig::http_client::Error),

    #[error("agent turn failed")]
    Prompt(#[from] PromptError),
}

impl AgentSession {
    pub fn new(config: AgentConfig) -> Result<Self, AgentError> {
        let model = config.model.trim();
        if model.is_empty() {
            return Err(AgentError::EmptyModel);
        }

        let workspace = Workspace::new(config.workspace_root.clone()).map_err(|source| {
            AgentError::Workspace {
                path: config.workspace_root.clone(),
                source: Box::new(source),
            }
        })?;

        let client = ollama::Client::builder()
            .api_key(rig::client::Nothing)
            .base_url(config.ollama_base_url)
            .build()?;

        let completion_model = client.completion_model(model);

        Ok(Self::with_model(completion_model, workspace))
    }

    /// The injection seam that makes agent wiring testable without a network
    /// call: `new` does all network-adjacent construction and delegates here,
    /// so tests can substitute `rig::test_utils::MockCompletionModel` and
    /// assert tool registration, `ToolContext` insertion, and reply
    /// extraction. See docs/decisions/0006-ollama-only-injectable-model.md.
    fn with_model<M>(model: M, workspace: Workspace) -> Self
    where
        M: CompletionModel + 'static,
    {
        let agent = rig::agent::AgentBuilder::new(model)
            .preamble(PREAMBLE)
            .memory(rig::memory::InMemoryConversationMemory::new().with_filter(recent_turns))
            .tool(ListDirectory)
            .tool(ReadFile)
            .default_max_turns(8)
            .build();

        Self { agent, workspace }
    }

    pub async fn prompt(&self, prompt: &str) -> Result<AgentReply, AgentError> {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return Err(AgentError::EmptyPrompt);
        }

        let mut context = ToolContext::new();
        context.insert(self.workspace.clone());

        let response = self
            .agent
            .prompt(trimmed)
            .conversation(CONVERSATION_ID)
            .tool_context(context)
            .extended_details()
            .await?;

        Ok(AgentReply {
            text: response.output,
        })
    }
}

/// Keeps only the most recent `HISTORY_TURNS` turns of a conversation.
///
/// Without this the history grows without bound: every prompt, reply, and tool
/// result is appended forever, so a long session eventually sends more than the
/// model can accept.
///
/// The cut has to land on a turn boundary rather than simply keeping the last N
/// messages. Rig represents a tool result as a user message, so a naive cut can
/// leave a tool result at the front of the history with the assistant tool call
/// it answers already dropped — a shape providers reject. A turn begins at a
/// user message that is not a tool result, and everything belonging to that
/// turn follows it, so cutting there always leaves the pairs intact.
fn recent_turns(messages: Vec<Message>) -> Vec<Message> {
    let turn_starts: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter(|(_, message)| is_turn_start(message))
        .map(|(index, _)| index)
        .collect();

    if turn_starts.len() <= HISTORY_TURNS {
        return messages;
    }

    let cut = turn_starts[turn_starts.len() - HISTORY_TURNS];
    messages.into_iter().skip(cut).collect()
}

fn is_turn_start(message: &Message) -> bool {
    matches!(
        message,
        Message::User { content }
            if !content
                .iter()
                .any(|item| matches!(item, UserContent::ToolResult(_)))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::test_utils::{MockCompletionModel, MockTurn};
    use tempfile::TempDir;

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime should build")
            .block_on(future)
    }

    fn workspace(dir: &TempDir) -> Workspace {
        Workspace::new(dir.path().to_path_buf()).expect("workspace root should be valid")
    }

    #[test]
    fn new_rejects_a_blank_model_name_without_touching_the_network() {
        let dir = TempDir::new().unwrap();
        let config = AgentConfig {
            model: "   ".to_string(),
            ollama_base_url: "http://127.0.0.1:1".to_string(),
            workspace_root: dir.path().to_path_buf(),
        };

        let err = AgentSession::new(config).err().unwrap();

        assert!(matches!(err, AgentError::EmptyModel));
    }

    #[test]
    fn new_reports_an_invalid_workspace_without_touching_the_network() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("missing");
        let config = AgentConfig {
            model: "qwen3".to_string(),
            ollama_base_url: "http://127.0.0.1:1".to_string(),
            workspace_root: missing,
        };

        let err = AgentSession::new(config).err().unwrap();

        assert!(matches!(err, AgentError::Workspace { .. }));
    }

    #[test]
    fn prompt_rejects_blank_input_without_touching_the_network() {
        let dir = TempDir::new().unwrap();
        let config = AgentConfig {
            model: "qwen3".to_string(),
            ollama_base_url: "http://127.0.0.1:1".to_string(),
            workspace_root: dir.path().to_path_buf(),
        };
        let session = AgentSession::new(config).expect("client construction is network-free");

        let err = block_on(session.prompt("   ")).err().unwrap();

        assert!(matches!(err, AgentError::EmptyPrompt));
    }

    #[test]
    fn dispatches_a_read_file_tool_call_against_the_configured_workspace() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hi there").unwrap();
        let ws = workspace(&dir);

        let model = MockCompletionModel::from_turns([
            MockTurn::tool_call(
                "call-1",
                "read_file",
                serde_json::json!({ "path": "hello.txt" }),
            ),
            MockTurn::text("The file says hi there."),
        ]);

        let session = AgentSession::with_model(model.clone(), ws);

        let reply = block_on(session.prompt("what does hello.txt say?")).unwrap();

        assert_eq!(reply.text, "The file says hi there.");

        let requests = model.requests();
        assert_eq!(
            requests.len(),
            2,
            "expected an initial call and a follow-up after the tool result"
        );
    }

    /// Guards I3 (docs/architecture.md): asserts the *exact* tool set, not
    /// merely that the read-only tools are among those registered, so an
    /// accidentally added write tool fails this test.
    #[test]
    fn offers_exactly_the_read_only_tools() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        let model = MockCompletionModel::from_turns([MockTurn::text("done")]);
        let session = AgentSession::with_model(model.clone(), ws);

        block_on(session.prompt("hi")).unwrap();

        let requests = model.requests();
        let mut tool_names: Vec<&str> = requests[0]
            .tools
            .iter()
            .map(|def| def.name.as_str())
            .collect();
        tool_names.sort_unstable();

        assert_eq!(tool_names, ["list_directory", "read_file"]);
    }

    fn turn_count(messages: &[Message]) -> usize {
        messages.iter().filter(|m| is_turn_start(m)).count()
    }

    #[test]
    fn recent_turns_keeps_a_short_history_untouched() {
        let history = vec![
            Message::user("one"),
            Message::assistant("first"),
            Message::user("two"),
            Message::assistant("second"),
        ];

        let kept = recent_turns(history.clone());

        assert_eq!(kept.len(), history.len());
    }

    #[test]
    fn recent_turns_drops_the_oldest_turns_beyond_the_cap() {
        let history: Vec<Message> = (0..HISTORY_TURNS + 4)
            .flat_map(|i| {
                [
                    Message::user(format!("prompt {i}")),
                    Message::assistant(format!("reply {i}")),
                ]
            })
            .collect();

        let kept = recent_turns(history);

        assert_eq!(turn_count(&kept), HISTORY_TURNS);
        assert_eq!(kept.first(), Some(&Message::user("prompt 4")));
    }

    /// Rig models a tool result as a user message, so a cut that ignored turn
    /// boundaries could open the history with a tool result whose assistant
    /// tool call had been dropped — a shape providers reject.
    #[test]
    fn recent_turns_never_starts_the_history_with_an_orphaned_tool_result() {
        let history: Vec<Message> = (0..HISTORY_TURNS + 4)
            .flat_map(|i| {
                [
                    Message::user(format!("prompt {i}")),
                    Message::assistant(format!("calling a tool for {i}")),
                    Message::tool_result(format!("call-{i}"), "read_file", "file contents"),
                    Message::assistant(format!("reply {i}")),
                ]
            })
            .collect();

        let kept = recent_turns(history);

        assert_eq!(turn_count(&kept), HISTORY_TURNS);
        assert!(
            is_turn_start(kept.first().expect("history is not empty")),
            "the history must begin at a real prompt, not a tool result"
        );
    }

    #[test]
    fn a_long_session_sends_a_bounded_history_to_the_model() {
        let dir = TempDir::new().unwrap();
        let ws = workspace(&dir);
        let turns: Vec<MockTurn> = (0..HISTORY_TURNS + 4)
            .map(|i| MockTurn::text(format!("reply {i}")))
            .collect();
        let model = MockCompletionModel::from_turns(turns);
        let session = AgentSession::with_model(model.clone(), ws);

        for i in 0..HISTORY_TURNS + 4 {
            block_on(session.prompt(&format!("prompt {i}"))).unwrap();
        }

        let requests = model.requests();
        let last = requests.last().expect("at least one request");
        // `HISTORY_TURNS` remembered turns plus the prompt being answered now,
        // which is appended to the loaded history rather than read from it.
        assert_eq!(turn_count(&last.chat_history), HISTORY_TURNS + 1);
    }
}
