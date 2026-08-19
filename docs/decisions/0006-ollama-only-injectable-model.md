# 0006. One provider (Ollama) for now, but swappable underneath

Status: Accepted
Date: 2026-08-18

## Context

The underlying agent framework `fey` is built on (Rig) already supports several different model
providers. It would be possible to build `fey`'s own abstraction on top — "any LLM backend, plug
one in" — but with only one provider actually in use, that abstraction would just be a guess at a
shape we don't have enough information to design well yet.

At the same time, the agent's wiring — which tools it registers, how it hands off to the model,
how it reads the reply back — needed to be testable without a real network call, since testing
against a live local Ollama is slow and its answers aren't always identical run to run.

## Decision

For now, `fey` only talks to one thing: a local Ollama server. The model name is required
(`--model` / `FEY_MODEL`) rather than defaulting to something, because there's no model name that
would be a sensible default across different machines. The server address defaults to Ollama's
usual local address but can be overridden.

Under the hood, though, the piece of code that actually builds the connection to the model is kept
as one small, swappable seam. Building the real Ollama connection is separated from everything
else the agent does, so tests can substitute a fake, scripted model in its place and check that
the right tools get registered and the right things happen — without that test depending on Rig's
own correctness, which isn't `fey`'s job to verify.

Conversations are kept in memory only, with no persistence between runs, and each turn of the
conversation is capped at a small number of tool-call round trips so the model can't loop forever.

## Consequences

- Don't build a general provider/plugin system before there's an actual second provider to design
  it against.
- Any new agent behavior — a new tool, a changed system prompt, showing tool calls as they happen
  — should get a test through that same swappable seam, the way the existing tool-dispatch test
  does. It's the one place a mistake in wiring things together gets caught before you find out the
  hard way in a real session, since a missing piece there fails silently rather than crashing.
- Nothing about a conversation survives restarting `fey`. Making it persist is a separate decision
  for later, not something this covers.
