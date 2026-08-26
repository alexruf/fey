# 0007. The agent only remembers the last few turns

Status: Accepted
Date: 2026-08-26

## Context

[ADR-0006](0006-ollama-only-injectable-model.md) settled that a conversation lives in memory for
as long as `fey` is running and disappears when it exits. What it didn't settle was how much of
that conversation gets sent back to the model on each new prompt, and the answer up to now was
"all of it".

That turns out to matter much more here than it would in a plain chat application, because this
agent reads files. A single `read_file` result can be tens of kilobytes, and every one of those
results stays in the history and gets re-sent with every subsequent prompt. So the request doesn't
grow gently with the length of the conversation — it grows with the total size of everything the
agent has ever looked at. After a handful of prompts that each read a few files, the request is
large enough that the model has to start dropping the beginning of it, and what gets dropped first
is the system prompt telling it what it is and what it's allowed to do. The failure isn't a clean
error, it's the agent gradually getting worse and more confused, which is a genuinely unpleasant
thing to debug.

## Decision

Only the most recent eight turns are sent to the model. Everything older is left out.

The important part is *where* the history gets cut, not how many turns are kept. Rig represents a
tool result as a message from the user — the same shape as a real prompt — which means simply
keeping the last N messages can leave a tool result sitting at the very start of the history with
the assistant message that requested it already gone. Providers reject that, and rightly so: it's
an answer to a question that isn't there. So the cut is always made at the start of a turn, which
is a user message that isn't a tool result. Everything belonging to a turn comes after its opening
prompt, so cutting at that point always leaves matched pairs behind.

Eight is not a measured figure. It's meant to be clearly more than anyone follows in a single line
of questioning while still being small enough that the size of the history stays in a sane range.

## Consequences

- The agent will forget. Ask about something from fifteen prompts ago and it won't know what
  you're referring to. That's a real limitation and the honest tradeoff for staying inside the
  model's context: the alternative isn't a longer memory, it's an agent that degrades unpredictably
  once the limit is hit.
- Because the cut is done by a filter applied when history is loaded, rather than by discarding
  messages as they arrive, nothing is actually destroyed. The full conversation is still there in
  memory, so summarising older turns instead of dropping them — which is the obvious next step if
  forgetting turns out to be annoying in practice — can be added in the same place without
  restructuring anything.
- Any future change to how history is shaped has to keep the turn-boundary rule. The tests cover
  this directly, including the orphaned-tool-result case, because it's the kind of mistake that
  produces a confusing provider-side error rather than an obvious local failure.
