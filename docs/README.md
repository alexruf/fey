# Documentation

If you're trying to understand *why* `fey` is built the way it is, this is the place — the code
itself is the best source for *what* it does.

- **[`architecture.md`](architecture.md)** — start here. How the codebase is split into two
  halves, what happens when you send a prompt, and the rules the code has to keep obeying.
- **[`decisions/`](decisions/)** — the individual decisions behind those rules, one per file,
  written down at the time they were made.
- **[`testing.md`](testing.md)** — how to manually check the parts that automated tests can't
  reach, mainly anything that needs a live terminal or a running Ollama.
- **[`roadmap.md`](roadmap.md)** — what's likely to come next, and why it isn't here yet.

## Adding a decision record

When you (or an AI agent working on this repo) make a design decision worth remembering, write it
down as a new file in `decisions/`, numbered one higher than the last one. Use this shape:

```markdown
# NNNN. Title

Status: Accepted | Superseded by NNNN
Date: YYYY-MM-DD

## Context

What situation led to this decision — what problem or tension was actually being solved.

## Decision

What was chosen, in plain terms.

## Consequences

What this makes possible, and — usually more useful in practice — what it now rules out.
```

The one rule that matters here: **once a decision record is written, don't edit it to match
something that changed later.** If the code moves on, write a *new* record that explains why, and
mark the old one as superseded by it. That's what keeps these useful — each one is a snapshot of
what was true and why, at the time, so it can never quietly go stale the way a description of
"how things currently work" can.

`architecture.md` is the exception: it's meant to always describe *today's* rules, so it should be
updated directly when one of those rules changes (and that change is usually itself worth a new
decision record explaining why).
