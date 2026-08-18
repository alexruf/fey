# AGENTS.md

> This file provides development guidelines for AI coding agents working with this codebase. It follows the open [AGENTS.md standard](https://agents.md/).

## Project Context

`fey` is a minimal, terminal-native AI coding agent written in Rust. Read [README.md](README.md) for details.

The crate is split into a binary and a library, per the UI-independent-core-vs-presentation rule:

- `src/lib.rs` — UI-independent core logic
- `src/main.rs` — CLI entry point / terminal presentation

Both are currently stubs; no functionality has been implemented yet.

### Commands

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
```

Run a single test: `cargo test --locked <test_name>`

Rustup is required; `rust-toolchain.toml` pins Rust 1.97.1 with Rustfmt and Clippy. `nix develop` provides the same toolchain from the locked flake as an optional alternative to rustup.

## Development Guidelines

### Philosophy

- Incremental progress over big bangs: small, working, committed increments
- Choose the boring, obvious solution; no premature abstractions
- Single responsibility per function/module
- If it needs explanation, simplify it

### Before Writing Code

1. **Explore** — Find similar implementations in the codebase
2. **Understand** — Identify existing patterns, conventions, and utilities; match them
3. **Plan** — Break complex work into small, testable increments
4. **Verify APIs** — Check library APIs and signatures against the installed version or docs; never guess from training knowledge

### Implementation Cycle

1. **Test** — Write failing test first for production code; skip for exploratory/scripting work
2. **Implement** — Minimal code to pass
3. **Refactor** — Clean up while tests pass
4. **Commit** — Small, working increments

### Scope Discipline

- Change only what the task requires; no drive-by refactoring or reformatting of unrelated code
- If you notice unrelated issues: report them, don't fix them unprompted
- No new dependencies without stating why; prefer stdlib and existing project dependencies

### When Stuck

**Stop after 3 failed attempts.** Then: document what failed, research 2–3 alternatives, question the abstraction level, ask or try a different angle. Never brute-force by trial and error.

### Communication

- Lead with the action or result, not the reasoning
- After multi-file changes: brief summary of what changed and why — no prose walkthrough of the process
- Don't ask for confirmation on routine edits
- Always ask before destructive or hard-to-reverse actions: force push, deleting files/branches, database migrations, rewriting history

### Code Standards

- Composition over inheritance; dependency injection over singletons; explicit data flow over hidden state
- Fail fast with descriptive errors; include debugging context; never silently swallow errors
- Never commit secrets, credentials, or tokens; use env/config mechanisms
- Every commit: builds, introduces no new test failures, includes tests for new behavior, passes formatter and linter
- Pre-existing test failures: report them, don't fix or skip them unprompted
- Never use `--no-verify`; never disable or skip tests instead of fixing them
- Commits follow Conventional Commits unless the project specifies otherwise; imperative mood; write the "why"

### Decision Framework

When choosing between approaches, prioritize in order:
1. Testability  2. Readability  3. Consistency with existing patterns  4. Simplicity  5. Reversibility

### Testing

- Test behavior, not implementation details; one concept per test
- Descriptive names (given/when/then); deterministic — no flaky tests
- Use existing test utilities before writing new ones

### Documentation

- After every change or new feature: check whether README.md, AGENTS.md, and any other affected docs still describe current behavior accurately; update what's stale, add what's missing
- Treat outdated documentation as a defect, not an afterthought
