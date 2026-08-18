# fey

`fey` is a minimal, terminal-native AI coding agent written in Rust.

## Development

Rustup is required. `rust-toolchain.toml` selects Rust 1.97.1 with Rustfmt and Clippy.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

## Optional Nix development shell

`nix develop` provides the same Rust toolchain from the locked flake. Nix is not required for the standard Cargo workflow. Future native dependencies belong in `flake.nix`.
