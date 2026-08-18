use std::io::{self, BufRead, Write};

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

    let session = AgentSession::new(AgentConfig {
        model: cli.model,
        ollama_base_url: cli.ollama_url,
        workspace_root,
    })?;

    tokio::runtime::Runtime::new()?.block_on(repl(session))
}

async fn repl(session: AgentSession) -> anyhow::Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let reply = session.prompt(&line?).await?;
        println!("{}", reply.text);
        io::stdout().flush()?;
    }
    Ok(())
}
