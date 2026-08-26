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
3. **Making the terminal nicer to look at and to type into.** The live region is two plain rows
   today — a `› ` prompt and a footer that is one of three fixed strings — and every wrapped line
   of a message is painted in a single colour, role label included. There's a lot of cheap polish
   available here: styling the label differently from the text beside it, indenting the
   continuation lines of a long answer so it doesn't read as several unrelated paragraphs, and
   showing that a turn is actually progressing rather than a motionless `Thinking…`. The input
   line needs the same attention: it currently understands printable characters, Backspace,
   Enter, paste and Ctrl-C, and nothing else — no moving the cursor back into what you typed, no
   word-wise deletion, no recalling the previous prompt. Two things constrain all of it. A
   message is written into scrollback once and never touched again, so how it looks has to be
   decided before it's written; and none of this may be done by printing escape codes straight to
   the terminal — it goes through the same wrapping and styling path everything else uses, see
   [ADR-0002](decisions/0002-inline-viewport-native-scrollback.md). Worth doing next to point 2,
   since both change the same function.
4. **Letting the agent write files or run commands.** This is a much bigger step than it sounds —
   it needs its own decision about how the user approves risky actions before they happen. It's
   deliberately not something to bolt onto the current read-only design; see
   [ADR-0004](decisions/0004-read-only-workspace-sandbox.md).
