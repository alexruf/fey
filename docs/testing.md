# Manual QA playbook

The 60 or so tests that live next to the code check individual pieces in isolation — no network,
no real terminal. What follows is what those tests can't reach: things you have to actually run
and watch. Do this before a release, or after touching `src/agent.rs` or anything under
`src/tui/`.

## Prove it's actually read-only

Run `git status --short` right before a live session and again right after. The two outputs
should be byte-for-byte identical. This is the simplest real proof that talking to the agent
didn't create, change, or delete anything in the workspace.

## Live smoke test

With a local Ollama running a tool-calling model:

```sh
cargo run --locked -- --model <tag>
```

1. Ask: `List the files at the workspace root, then read Cargo.toml and tell me the package name
   and direct dependencies.` The answer should correctly name the package, cite `Cargo.toml`, and
   only mention dependencies it actually read through the tools — not ones it's guessing at.
2. Follow up with: `What did I ask you to inspect?` — this checks that the conversation actually
   remembers the previous turn.
3. While the model is thinking, the app should stay responsive — you should still be able to see
   it redraw and still be able to press Ctrl-C.

## Things to check by hand in the terminal itself

Each of these deserves its own separate run:

- **Scrollback.** Send enough prompts to push earlier messages off the top of the screen, then
  scroll up using your terminal's own scrolling (not any key inside the app) and confirm they're
  still there.
- **Wide characters.** Type a wide character like a CJK character, then backspace it. It should
  render and delete cleanly, with no crash.
- **Resizing the window** mid-conversation shouldn't crash anything. Some visual glitches during
  the resize itself are a known, accepted rough edge (see ADR-0002) — just don't expect a crash.
- **Ctrl-C.** Try it once while idle, and once while a prompt is still in flight, in two separate
  runs. Either way, the terminal should end up back in a normal, usable state — a visible cursor,
  no leftover prompt line or status line still sitting on screen — and whatever was already
  printed to scrollback should still be there afterward.
- **The model being unreachable.** Stop Ollama, or point `--ollama-url` somewhere that doesn't
  exist, then send a prompt. The error should show up in the scrollback, and the app should go
  back to a normal, ready-to-type state afterward — not get stuck.

### If you're scripting this instead of doing it by hand

If you try to drive the TUI from a script using a bare pseudo-terminal (for example, Python's
`pty.openpty()`), you'll likely hit an error like *"the cursor position could not be read within
a normal duration"* when the app tries to exit. That's not a bug in `fey` — it happens because
exiting the app asks the terminal "where's your cursor right now?" (a standard terminal query),
and a bare pseudo-terminal with nothing else attached never answers that question. A real
terminal emulator always does. If you need to script this, your script has to answer that query
itself the moment it sees it.

## A known source of flakiness (not a `fey` bug)

Some local Ollama models occasionally send back a tool call that Ollama itself can't parse, and
Ollama responds with a server error mentioning "XML syntax error." This is something going wrong
inside Ollama or the model, not inside `fey` — `fey` handles it correctly: the error shows up in
scrollback and the app returns to normal. It just doesn't happen every time you ask the same
question. If a smoke test fails this way, try it again before assuming something actually broke.
