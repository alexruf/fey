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

Whatever folder you run `fey` from becomes its workspace: it can look through files and folders
inside it, but it can't create, edit, or delete anything, and it can't run shell commands.

The interface stays out of your way: only the input line and a status line at the bottom are
"live." Everything else — your messages and the model's replies — is written straight into your
terminal's normal scrollback, so scrolling back and copying text just works the way it always
does in your terminal.

| Key | Action |
| --- | --- |
| Type | Compose a single-line prompt |
| Enter | Submit the prompt |
| Ctrl-C | Quit (aborts an in-flight prompt immediately) |

## Development

Rustup is required. `rust-toolchain.toml` selects Rust version with Rustfmt and Clippy.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

## Optional Nix development shell

`nix develop` provides the same Rust toolchain from the locked flake. Nix is not required for the standard Cargo workflow. Future native dependencies belong in `flake.nix`.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache License, Version 2.0](LICENSE-APACHE) at your option.
