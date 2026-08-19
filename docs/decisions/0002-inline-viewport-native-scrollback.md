# 0002. Live in your terminal's own scrollback, don't build a new one

Status: Accepted
Date: 2026-08-19

## Context

A coding assistant produces a lot of back-and-forth over a session, and there are two very
different ways to show that. One is to take over the whole screen — an "alternate screen" app,
like `vim` or `htop` — and build your own scrollable history inside it. The other is to stay
inline at the bottom of the terminal and just let the terminal itself keep the history, the way a
normal shell session does.

We chose the second option. The first one means reimplementing scrolling, searching, and text
selection — things every terminal emulator already does well — and it also means the conversation
doesn't show up in things people already rely on, like `tmux`'s own scrollback or a screen
reader.

## Decision

`fey` uses a small, fixed two-line strip at the bottom of the terminal for the input line and a
status footer — nothing else lives there. Every finished message (what you typed, what the model
replied, any error) gets written into the terminal's normal output exactly once, and is never
redrawn or touched again. The app itself doesn't keep a copy of the conversation; your terminal's
own scrollback *is* the history.

Two dependencies that were originally listed as "we'll probably want these eventually" got
removed instead of kept around: one writes progress bars straight to the terminal, which can't
coexist with an app that owns that same output; the other renders markdown directly, which would
bypass the one function every displayed message is supposed to go through.

## Consequences

- The app's internal state never needs to track a scroll position or hold onto old messages —
  there's nothing to scroll *inside* the app, because there's nothing kept in memory to scroll
  through.
- Every message that gets displayed has to go through the same formatting function on its way to
  the screen. That's deliberate — it's what makes that function simple to test on its own.
- When markdown rendering is eventually added, it needs to happen inside that same formatting
  step, as styled text — not by printing raw formatting codes straight to the terminal.
- Multi-line input (composing a longer prompt, or pasting multiple lines) isn't supported yet —
  the input line is built for a single line only.
