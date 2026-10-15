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

Planning for individual changes lives outside this folder, in OpenSpec:

- **[`../openspec/specs/`](../openspec/specs/)** — the maintained specifications of how parts of
  `fey` behave. They only exist for areas someone has specified so far, so an empty or missing
  spec doesn't mean the behavior is undefined; the code and tests still describe the rest.
- **[`../openspec/changes/`](../openspec/changes/)** — work in progress. Each change has its own
  folder with a proposal, an optional design, a task list, and *delta specs* that describe what the
  change would add to, modify in, or remove from the main specs. A delta only becomes part of the
  main specs when the change is synced or archived. Finished changes move to `archive/`.

## Choosing between OpenSpec and a direct change

OpenSpec is worth its overhead when writing things down first actually helps: when the
requirements need to outlive the conversation, when there are real alternatives to weigh, when
several people or steps need to coordinate, or when the contract itself is still unclear. A new
capability with nontrivial behavior, a change that cuts across the codebase, or a migration are
typical examples. Size alone is a poor guide. A three-line diff that loosens the workspace
sandbox deserves more thought than a large but mechanical rename.

When the scope, the expected behavior, and the approach are already clear, just make the change.
Fixing a typo, refactoring a function without changing what it does, or fixing a bug so the code
matches behavior that was already established all fall into this group. They need no change
folder, proposal, design, task list, or new spec.

This is a recommendation, not a gate. An AI agent may suggest OpenSpec once and say why, but it
only creates OpenSpec files when you ask for them or agree to the suggestion. If you'd rather go
straight to the code, it does that without bringing the suggestion up again. Skipping OpenSpec
never skips anything else: investigating first, writing tests, verifying the result, asking before
adding dependencies, staying in scope, keeping documentation current, and writing a decision
record when one is needed all still apply.

A direct change still has to keep any existing main spec it affects accurate. If the change only
brings the code back in line with a spec, the spec is already right and stays as it is. If it
changes specified behavior, update the main spec in the same change. Before you do, check the
active changes for a delta spec that touches the same requirement, and update that delta too.
Syncing or archiving a change replaces the requirement with the delta's version, so an edit made
only to the main spec would quietly disappear. If working out the new requirements turns into a
real discussion, that's the signal to switch to OpenSpec. A small change doesn't need to write a
spec for an area that has none yet.

Keep in mind what each kind of document is for, so the same thing isn't written down twice:

- Main specs describe behavior that is maintained today; delta specs describe behavior that is
  only proposed.
- A change's proposal, design, and tasks explain why that one change exists and how it is meant
  to be built. A design can keep changing while the work is in progress. Finishing the planning
  artifacts, ticking off the tasks, or archiving the change doesn't prove the code works; only
  the tests and verification do. An archived change is a record of how that change came about,
  not a description of the current architecture.
- Decision records in [`decisions/`](decisions/) capture architectural choices that outlast any
  single change.
- [`architecture.md`](architecture.md) describes the boundary and invariants as they are today.

A proposal or design can't overrule an accepted decision record just by existing. If it needs to
go against one, it has to plan a new decision record that supersedes the old one, plus the
matching update to `architecture.md`.

## Adding a decision record

Decision records exist so that the reasons behind the architecture survive longer than the
conversation that produced them. Whether a change needs one has nothing to do with whether it
went through OpenSpec: an OpenSpec feature may need no record at all, and a one-line direct change
may need one.

Write a record when a change commits the project to something later work has to live with: a new
or redrawn boundary between parts of the code, or a lasting policy such as how tools may access
the workspace or how tightly `fey` is tied to one model provider. Any change to an invariant in
`architecture.md` always needs one, including widening the library's public API. Routine choices,
such as which algorithm to use, how to structure helpers, or details that only matter inside one
feature, don't need a record. They belong in the change's OpenSpec design if there is one, and
otherwise need no separate write-up. If an existing record already covers the decision, point
to it instead of writing another.

When a change has both an OpenSpec design and a decision record, keep the record short and let
it stand on its own: the decision, enough context to understand it, and its consequences. The
alternatives that were considered, the task breakdown, and the detailed requirements stay in the
change, and the design links to the record rather than repeating it.

A record never links to an active change. That change isn't implemented yet, its plan may still
shift, and its folder moves when it's archived, so the link would break or point at something
that no longer matches. The record can mention the change by name, though; the name survives in
the archive folder, and the change's design links back to the record. A record may link to an
already archived change as background, as long as it still makes sense without following the
link. Don't go back and add an archive link once the change is archived — that would be editing a
finished record.

To add one, create a new file in `decisions/`, numbered one higher than the last one. Use this
shape:

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
