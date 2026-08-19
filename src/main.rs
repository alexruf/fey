mod tui;

use clap::Parser;
use fey::{AgentConfig, AgentSession};

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, env = "FEY_MODEL")]
    model: String,

    #[arg(
        long,
        env = "OLLAMA_API_BASE_URL",
        default_value = "http://localhost:11434"
    )]
    ollama_url: String,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let workspace_root = std::env::current_dir()?;

    // Construct the session before entering the inline viewport so config,
    // workspace, or client errors surface as plain stderr output rather than
    // inside raw mode.
    let session = AgentSession::new(AgentConfig {
        model: cli.model,
        ollama_base_url: cli.ollama_url,
        workspace_root,
    })?;

    let runtime = tokio::runtime::Runtime::new()?;
    tui::run(&runtime, session)
}
