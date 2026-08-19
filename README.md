# fey

`fey` is a minimal, terminal-native AI coding agent written in Rust. This first milestone is
read-only: it can inspect the launch directory but cannot edit files or run commands.

## Usage

`fey` talks to a local [Ollama](https://ollama.com) server; install Ollama, pull a tool-calling
model, and make sure `ollama serve` is running before starting `fey`.

```sh
cargo run --locked -- --model <tag>
# or: FEY_MODEL=<tag> cargo run --locked
```

`--ollama-url` (or `OLLAMA_API_BASE_URL`) overrides the default `http://localhost:11434`.

The launch directory is the read-only workspace: `fey` can list directories and read files inside
it, but cannot create, modify, or delete anything, and cannot run shell commands.

The UI is an inline viewport at the bottom of the terminal; finalized messages are written once
into the terminal's own scrollback, so scrolling and copying use the terminal emulator itself.

| Key | Action |
| --- | --- |
| Type | Compose a single-line prompt |
| Enter | Submit the prompt |
| Ctrl-C | Quit (aborts an in-flight prompt immediately) |

## Development

Rustup is required. `rust-toolchain.toml` selects Rust 1.97.1 with Rustfmt and Clippy.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

## Optional Nix development shell

`nix develop` provides the same Rust toolchain from the locked flake. Nix is not required for the standard Cargo workflow. Future native dependencies belong in `flake.nix`.
