# 0003. Keep the terminal loop and the async runtime strictly separate

Status: Accepted
Date: 2026-08-19

## Context

Two things need to happen at once: reading keystrokes and redrawing the screen (which is ordinary,
synchronous code), and talking to Ollama over the network (which has to be asynchronous, since
it's waiting on an HTTP response). It's tempting to just put the whole app inside Rust's
`#[tokio::main]` and read keystrokes from within that async function. That actually works, most of
the time — but it works by accident: it only holds up because the runtime happens to have a spare
worker thread doing nothing else. If it didn't, that thread would sit there permanently doing
synchronous keyboard-polling instead of the async work it's supposed to be free for. It's also
just confusing to reason about, because "the terminal" and "the network call" end up tangled
together in the same function.

## Decision

Build the async runtime by hand instead of using the `#[tokio::main]` shortcut, and keep it
strictly separate from the terminal: one background task owns the entire conversation with the
model, and the actual keyboard/screen loop runs on the main thread, completely outside that
runtime. The two sides only ever talk to each other by dropping messages into a queue — the
terminal side sends "here's a prompt," the background task sends back "here's the reply" (or "it
failed"). Those queues are deliberately unbounded, because the terminal side is synchronous code
and literally cannot pause and wait the way a bounded queue would sometimes require it to.

The message the background task sends back is a small enum — "here's a reply" or "it failed" —
rather than Rust's usual `Result` type. That's so a third kind of message (a live update while the
model is still working, see [roadmap](../roadmap.md)) can be added later without having to touch
every place that currently only expects success or failure.

In practice, only one prompt is ever in flight at a time — the app simply won't let you send a
second one until the first one finishes — so those unbounded queues never actually build up.

## Decision detail

Since nothing uses the `#[tokio::main]` shortcut, the project doesn't need Tokio's `macros`
feature at all — only the pieces needed to build a runtime by hand and use those message queues.

## Consequences

- Don't "simplify" this by switching the message queues to bounded ones without also rethinking
  the whole loop — the reason they're unbounded is load-bearing, not incidental.
- Don't reach for `#[tokio::main]` here later; it would bring back exactly the problem this
  decision avoids.
- If you press Ctrl-C while a prompt is still being answered, the background task gets cancelled
  and the app exits right away — it doesn't wait for the network call to finish first. (There's no
  real "Ctrl-C signal" while the terminal is in this mode; it just shows up as an ordinary
  keypress that the app has to notice and act on.)
