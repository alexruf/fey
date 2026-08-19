//! Guards I1 (docs/architecture.md): nothing terminal-related is reachable
//! from `fey`'s public API, and the API surface is exactly the four types
//! ADR-0001 names.
//!
//! Two checks, because neither alone covers the invariant: the shape check
//! below would happily pass if `src/lib.rs` re-exported a fifth, terminal-only
//! type alongside the four; the source-line check only guards the export
//! list itself, not what each exported type is made of. Together they cover
//! both directions.
//!
//! The source-line check is a text assertion on `src/lib.rs`, not real API
//! reflection — a genuinely complete guard would need `cargo public-api`
//! (an external tool built on nightly rustdoc JSON). That's deliberately not
//! introduced here: `src/lib.rs` is a 7-line single gate, and rustfmt keeps
//! its formatting stable, so a line-based check is proportionate.

use std::path::PathBuf;

use fey::{AgentConfig, AgentError, AgentReply, AgentSession};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn public_types_are_built_from_plain_std_types_and_are_send_sync() {
    // Constructing (or destructuring) each type using only std types fails to
    // compile if a field's type ever becomes Ratatui-, Clap-, or otherwise
    // terminal-specific.
    let _config = AgentConfig {
        model: String::from("qwen3"),
        ollama_base_url: String::from("http://localhost:11434"),
        workspace_root: PathBuf::from("."),
    };
    let AgentReply { text } = AgentReply {
        text: String::from("hi"),
    };
    assert_eq!(text, "hi");

    assert_send_sync::<AgentConfig>();
    assert_send_sync::<AgentError>();
    assert_send_sync::<AgentReply>();
    assert_send_sync::<AgentSession>();
}

#[test]
fn lib_rs_exports_exactly_the_four_documented_core_types() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lib_rs = std::fs::read_to_string(format!("{manifest_dir}/src/lib.rs"))
        .expect("src/lib.rs should be readable");

    let pub_lines: Vec<&str> = lib_rs
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("pub "))
        .collect();

    assert_eq!(
        pub_lines.len(),
        1,
        "expected exactly one `pub` item in src/lib.rs (the four-type re-export), found: {pub_lines:?}"
    );
    assert_eq!(
        pub_lines[0],
        "pub use agent::{AgentConfig, AgentError, AgentReply, AgentSession};"
    );
}
