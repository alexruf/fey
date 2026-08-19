# Roadmap

Nothing here is scheduled or promised — this is just the order things make the most sense to
tackle in, given how the code is currently shaped.

## What's likely to come next

1. **Showing what the agent is doing, not just "Thinking…".** Right now, if the model reads three
   files before answering, you just see a spinner-like status for however long that takes — which
   can be tens of seconds. That's the biggest gap in the experience today. The fix is to surface
   each tool call as it happens instead of only showing the final answer. The code is already set
   up for this: the message that comes back from the background worker is a small, extensible
   type specifically so a new kind of message (a tool-call notification) can be added later
   without having to change everything that touches it (see
   [ADR-0003](decisions/0003-sync-terminal-loop-explicit-runtime.md)).
2. **A third read-only tool**, like search/grep. This is a small, well-worn addition — the
   existing `list_directory` and `read_file` tools are the template to copy.
3. **Rendering assistant replies as actual formatted text** (bold, lists, code blocks) instead of
   plain text. This has to happen inside the same function that already wraps message text for
   display, not by writing raw formatting codes straight to the terminal — see
   [ADR-0002](decisions/0002-inline-viewport-native-scrollback.md) for why that boundary matters.
4. **Letting the agent write files or run commands.** This is a much bigger step than it sounds —
   it needs its own decision about how the user approves risky actions before they happen. It's
   deliberately not something to bolt onto the current read-only design; see
   [ADR-0004](decisions/0004-read-only-workspace-sandbox.md).
