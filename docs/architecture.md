# Architecture

This is a map of the rules the code has to keep obeying, not a description of how it's currently
built — the module layout itself is easier to just read from the source. If one of these rules
changes, update this file to match; if the code just gets refactored without changing what it's
promising, this file shouldn't need to change with it.

## Boundary

`fey` is a single Cargo package, but it's built as two halves that don't trust each other equally:
a **core** that knows nothing about terminals, and a **presentation** layer that knows nothing
about how the agent actually works internally. Why bother with this split in a one-binary
project? Because it's what let the read-only sandbox and the agent wiring get fully tested from
day one, without ever touching a real terminal or a running model — see
[ADR-0001](decisions/0001-core-presentation-split.md) for the full reasoning.

Concretely:

- The **core** — `src/lib.rs` and the `agent`, `tools`, and `workspace` modules under it — is
  everything that doesn't care whether it's driven by a terminal, a test, or something else
  entirely. It only ever exposes four public types: `AgentConfig`, `AgentError`, `AgentReply`,
  `AgentSession`. Nothing else is meant to be reachable from outside.
- The **presentation** layer — `src/main.rs` and everything under `src/tui/` — owns the terminal:
  parsing CLI flags, running the async runtime, reading keystrokes, and drawing the screen. It's
  allowed to depend on the core. The core is never allowed to depend on it.

## Request path

Here's what happens between you pressing Enter and seeing a reply.

The terminal itself runs on the main thread and is entirely synchronous — it just reads
keystrokes and draws the screen. Talking to Ollama, on the other hand, is a network call and has
to be asynchronous. `fey` keeps these two worlds apart rather than mixing them: a single
background task ("the worker") owns the conversation with the model, and the terminal only ever
talks to that worker by dropping messages into a queue and picking up replies from another queue
— it never waits on the network directly. This is also why nothing can lock up the UI: the
terminal keeps redrawing and can still see your Ctrl-C even while a reply is being generated.

So a prompt takes two hops:

1. **You type and hit Enter.** Your message is written straight into the terminal's own
   scrollback right away — you see it before the model has even been asked anything. Then it's
   handed to the worker.
2. **The worker asks the model.** It calls the agent, which may read a file or list a directory
   along the way (always through the sandboxed workspace, never the raw filesystem — see
   [ADR-0004](decisions/0004-read-only-workspace-sandbox.md)), and eventually gets back a final
   answer or an error. That result is handed back to the terminal, which writes it into
   scrollback the same way your own message was written.

There's exactly one place this crosses from "terminal code" into "agent code": a single function
call, `AgentSession::prompt` in `src/agent.rs`. Everything before that call belongs to the TUI
(`src/tui/`); everything from that call onward is the library core. See
[ADR-0003](decisions/0003-sync-terminal-loop-explicit-runtime.md) for why the two sides are kept
this strictly separate, and why only one prompt is ever allowed to be in flight at a time.

## Invariants

These are the rules that should never quietly stop being true. Each one links back to the ADR
that explains why it exists, and notes how well it's actually guarded today — some are guaranteed
by the type system, some just happen to be true because of how the code is currently written, and
a couple are weaker than they should be.

| # | Rule | Why (ADR) | How well it's guarded |
| --- | --- | --- | --- |
| I1 | Nothing from the terminal/UI world (Ratatui, Clap, raw terminal types) is visible outside `src/lib.rs` — only the four core types are | [0001](decisions/0001-core-presentation-split.md) | Guarded by `tests/public_api.rs`: the four types' shape and the single re-export line in `src/lib.rs` |
| I2 | Every tool that touches the filesystem goes through `Workspace` and can never resolve to a path outside it | [0004](decisions/0004-read-only-workspace-sandbox.md) | Well tested, including a symlink-escape test |
| I3 | The agent only ever offers the model read-only tools — nothing that writes or runs a command | [0004](decisions/0004-read-only-workspace-sandbox.md), [0006](decisions/0006-ollama-only-injectable-model.md) | Guarded by a test asserting the exact tool set |
| I4 | A failure the model can't do anything about (disk error, oversized file) never turns into a raw error message shown to the model | [0005](decisions/0005-tool-error-visibility.md) | Partial — see below |
| I5 | The TUI never keeps its own copy of the conversation; a message is written to the terminal once and never redrawn | [0002](decisions/0002-inline-viewport-native-scrollback.md) | Guaranteed by how the state is structured, plus tests |
| I6 | The terminal never blocks waiting on the network, and at most one prompt is ever in flight at a time | [0003](decisions/0003-sync-terminal-loop-explicit-runtime.md) | Guaranteed by how the state is structured |
| I7 | However `fey` exits — quit, crash, a failed request — the terminal is always left in a normal, usable state | [0003](decisions/0003-sync-terminal-loop-explicit-runtime.md) | Guaranteed — this runs on every exit path |

One of these is weaker than the table above lets on, and it's worth knowing why:

- **I4** has a real gap: error messages the model is allowed to see currently do sometimes include
  the full absolute file path on your machine, not just a path relative to the workspace. That's
  not the end of the world, but it wasn't the intent — see
  [ADR-0005](decisions/0005-tool-error-visibility.md) for the honest account of how this happened
  and [roadmap.md](roadmap.md) for the fix.
