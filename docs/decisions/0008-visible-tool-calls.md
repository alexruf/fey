# 0008. Show each tool call while the turn is still running

Status: Accepted
Date: 2026-08-26

## Context

Until now a prompt produced exactly one visible thing: the final answer. If the model listed a
directory and read two files first, all you saw in the meantime was the word "Thinking…" — often
for the better part of a minute. That was the worst part of using `fey`, because there was no way
to tell a slow answer apart from a stuck one.

Fixing it needs two questions answered. First, how does the core say what it's doing, given that
it isn't allowed to know a terminal exists? Second, how do those notifications reach the screen in
the right order relative to the answer they lead up to — a tool line printed *after* the reply it
preceded would be worse than not printing it at all.

For the first question, Rig offers three ways to see inside a turn. Its streaming API would mean
restructuring the whole reply path to consume a stream instead of awaiting a result, which is a
much larger change than this feature justifies. Its tracing/telemetry output is shaped for logs,
not for a UI, and says nothing about ordering. Its hooks are the remaining option, and they fit:
a hook is called with the tool's name and arguments just before that tool runs.

## Decision

`AgentSession::prompt` takes a second argument: a function the core calls once per tool call,
just before that tool runs. The core hands it a `ToolCall` — the tool's name, plus the string
arguments the model sent — and does nothing else with it.

The observer belongs to a single prompt rather than to the session, because whoever asked the
question is also who wants to see the progress; tying it to the turn means it can be routed to
wherever that turn's output goes, and it's dropped when the turn ends.

That answers the ordering question too, by not creating a second path for these messages. The
terminal's observer just drops each tool call into the same queue it already picks replies up
from, so a tool call and the reply after it are two items in one queue, in the order they were
produced. A separate queue for progress would have made the ordering between the two a matter of
which queue the loop happened to check first.

The notification only carries the *string* arguments a call was made with. That's what says
something a person cares about — the path being read, the folder being listed, later on a search
term. A paging offset is a number, and printing it would only add noise to the one line the user
sees.

Tool calls are written into the scrollback like any other message, not shown as a status that
disappears. The status line already says "Thinking…"; what was missing was a record of what
actually happened, and that's worth keeping around after the answer arrives.

## Consequences

- The core's public API is now five types rather than four: `ToolCall` joins the other four. This
  changes the letter of invariant I1 in [`architecture.md`](../architecture.md), not its point —
  nothing terminal-shaped crosses the boundary, and `ToolCall` is a name and some strings.
- The observer is called from inside the turn, on the worker's thread, so it must not block. In
  the terminal's case it does one non-blocking queue send and returns. Anything slower there would
  stall the agent itself.
- Rig's hooks can also rewrite a tool's arguments, skip the call, or stop the run. This
  deliberately uses none of that: watching the agent work must never be able to change what the
  agent does. If approval-before-action is ever wanted, that's the subject of its own decision
  (see [ADR-0004](0004-read-only-workspace-sandbox.md)), not an extension of this one.
- Every tool call the model makes is now visible, including ones that fail — a path outside the
  workspace shows up as an attempt, followed by the model correcting itself. That's information,
  not a defect, but it does mean the transcript shows the model's false starts.
- Showing the current tool in the live status line instead of, or as well as, the scrollback is
  now purely presentation-side work: the core already reports everything needed for it.
