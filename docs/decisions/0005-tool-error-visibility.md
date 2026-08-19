# 0005. Only some tool errors should be shown to the model

Status: Accepted
Date: 2026-08-17

## Context

When a tool call fails, there are really two different kinds of failure. Some are things the
model can fix by trying again differently — it asked for a path that doesn't exist, or a line
number past the end of the file, or a path that reaches outside the project. Others are things the
model has no way to act on at all — a disk error, a file that's too large, a file that isn't valid
text. Showing the model a raw, technical message makes sense for the first kind (it needs the
detail to correct itself) and is pointless — or worse, a way to accidentally leak information —
for the second.

## Decision

There's a single function (`map_workspace_error`, in `src/tools/mod.rs`) responsible for deciding
which is which, every time a filesystem error happens:

- Bad or malformed paths, a request that reaches outside the project, or a file/folder that
  doesn't exist — the model sees a real, specific message, so it can correct its next attempt.
- Everything else — disk errors, oversized files, files that aren't valid text — the model only
  gets a generic, stable message. The full detail is still available to whoever's running `fey`,
  just not shown to the model.

## Consequences

- Any new kind of filesystem error has to be deliberately sorted into one of these two groups when
  it's added — it shouldn't be left to fall through to whichever one happens to be the default.
- **Being honest about a gap here, rather than quietly leaving it out:** the messages in the first
  group — the ones the model *is* allowed to see — currently do still include the full absolute
  path on your machine, not just a path relative to the project. That wasn't the intent, but it's
  what the code does today. Making those messages workspace-relative instead is tracked as a small
  fix in [the roadmap](../roadmap.md). Until then, don't assume a model-visible error is free of
  local file-path detail — only the second group (the generic, stable messages) actually is.
- There are no automated tests covering this mapping yet, including the gap above — it's worth
  adding tests for both once the fix above lands.
