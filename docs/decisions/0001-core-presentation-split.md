# 0001. One package, split into a core half and a terminal half

Status: Accepted
Date: 2026-08-17

## Context

From the very first commit, `fey` needed a clean line between "the logic that talks to the model
and reads files" and "the code that draws a terminal UI." Without that line, it's hard to test
the sandboxing and tool behavior without also spinning up a terminal or a real model — and it's
easy for terminal-specific code to sneak into logic that has no business knowing a terminal
exists.

We also considered splitting this into multiple Cargo packages (a proper workspace) instead of
just organizing one package into two halves. We decided against it: that adds real overhead —
separate versioning, a publish/release boundary — for a project that has exactly one binary and no
second thing that would ever consume the library half on its own.

## Decision

Keep it as one package, but treat it internally as two halves that don't trust each other equally:

- `src/lib.rs` and everything under it (`agent`, `tools`, `workspace`) is the core. It knows
  nothing about terminals. It exposes exactly four public types — `AgentConfig`, `AgentError`,
  `AgentReply`, `AgentSession` — and nothing else is meant to be used from outside it.
- `src/main.rs` is where the program starts, and everything under `src/tui/` is the terminal UI.
  Both of those are allowed to depend on the core. The core is never allowed to depend on them.

## Consequences

- No terminal-related type — not Ratatui, not Clap, nothing raw-mode-specific — is allowed to leak
  into the core's public API. When the agent replies, it just hands back plain text
  (`AgentReply { text: String }`); it's the terminal half's job to decide how that gets displayed.
- Because of this split, the core could be fully tested — including a fake model standing in for
  Ollama — without ever touching a real terminal or making a real network call. That's what let
  the sandboxing and the agent wiring both ship and get verified before the TUI existed at all.
- If `fey` ever grows a second front end (say, a non-interactive/batch mode), that's the point
  where splitting the core out into its own real package would start to pay off. Not before.
