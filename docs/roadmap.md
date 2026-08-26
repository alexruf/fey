# Roadmap

Nothing here is scheduled or promised — this is just the order things make the most sense to
tackle in, given how the code is currently shaped.

## What's likely to come next

1. **A third read-only tool**, like search/grep. This is a small, well-worn addition — the
   existing `list_directory` and `read_file` tools are the template to copy. Tool calls are now
   visible while a turn runs (see
   [ADR-0008](decisions/0008-visible-tool-calls.md)), so a new tool shows up in the transcript
   without any extra work, as long as the argument that says what it's looking at is a string.
2. **Rendering assistant replies as actual formatted text** (bold, lists, code blocks) instead of
   plain text. This has to happen inside the same function that already wraps message text for
   display, not by writing raw formatting codes straight to the terminal — see
   [ADR-0002](decisions/0002-inline-viewport-native-scrollback.md) for why that boundary matters.
3. **Letting the agent write files or run commands.** This is a much bigger step than it sounds —
   it needs its own decision about how the user approves risky actions before they happen. It's
   deliberately not something to bolt onto the current read-only design; see
   [ADR-0004](decisions/0004-read-only-workspace-sandbox.md).
