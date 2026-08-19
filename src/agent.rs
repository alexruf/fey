//! Ollama-backed agent session: wires the read-only tools to a Rig agent
//! behind a small, injectable-model API.

use std::path::PathBuf;

use rig::client::CompletionClient as _;
use rig::completion::{CompletionModel, Prompt, PromptError};
use rig::providers::ollama;
use rig::tool::ToolContext;
use thiserror::Error;

use crate::tools::{ListDirectory, ReadFile};
use crate::workspace::Workspace;

const CONVERSATION_ID: &str = "main";
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

    #[error("failed to initialize workspace at {path:?}: {source}")]
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
            .memory(rig::memory::InMemoryConversationMemory::new())
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
}
