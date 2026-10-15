# Design

## Context

See `proposal.md` for motivation and scope. `AGENTS.md` already defines investigation, test-first implementation, quality gates, and immutable ADRs. `docs/README.md` explains ADR authoring but currently describes its threshold only as a design decision worth remembering. `docs/architecture.md` owns the current architectural invariants.

OpenSpec uses the `spec-driven` schema. There are no main capability specs yet, and the separate content-search change contains both a detailed design and a task to write an ADR. That ADR task asks for feature-level limits and pagination semantics, which this policy places elsewhere, so it is amended here. The policy must otherwise work before and after main specs exist, without requiring migration of historical decisions or other plans.

A design document is included here because the overlapping documentation responsibilities and the two independent adoption thresholds need explicit decisions before editing the project guidance. This change declares `skip_specs: true` because it changes contributor guidance rather than product behavior.

## Goals / Non-Goals

**Goals:**
- Give contributors a proportional path from a small direct edit to a change requiring persistent requirements and design work.
- Make the OpenSpec and ADR thresholds independent and explain document ownership without duplicating full designs.
- Keep concise agent instructions consistent with human-facing guidance and examples.

**Non-Goals:**
- Require OpenSpec for every feature, bug fix, refactor, or documentation edit.
- Change architecture invariants, Rust behavior, quality gates, or existing ADR contents.
- Customize generated OpenSpec skills or introduce another policy document, workflow schema, or approval gate.
- Backfill a complete specification of existing behavior or revise the content-search plan beyond aligning its ADR tasks with this policy.

## Decisions

### 1. Choose OpenSpec based on planning value, not edit size alone

Document a direct path for small, well-understood work whose scope, expected behavior, and approach are already clear. Examples include a typo, a localized refactor preserving behavior, or a narrowly scoped bug fix restoring an established contract. These changes need no OpenSpec change directory, proposal, design, tasks, or new capability spec solely to document that work.

Recommend OpenSpec when persistent requirements, alternatives, coordination, or multiple implementation steps would materially help: for example, a new capability with nontrivial behavior, a cross-cutting change, an ambiguous contract, or a migration. A small diff that changes a security boundary is not automatically simple. Do not use a line-count cutoff or make OpenSpec mandatory for all new behavior.

If a direct change affects an existing capability spec, keep that spec accurate without requiring a new OpenSpec change merely to edit it. A fix restoring specified behavior does not need a spec rewrite. If resolving the requirements becomes substantial, switch to OpenSpec. Do not require a new main spec for every previously unspecified feature touched by a small change.

Before editing a main spec directly, check active changes for delta specs that touch the same requirement. If one exists, update that delta as well. Syncing or archiving a change replaces the requirement with the delta's version, so a direct edit made only to the main spec would be silently discarded.

The criteria are advice, never a gate. An agent may recommend OpenSpec when they apply, once and with its reason, but it creates OpenSpec artifacts only when the user asks for them or accepts the recommendation. When the user asked for implementation or declines OpenSpec, the agent proceeds directly without re-raising the suggestion, refusing, or delaying the work. Every quality rule and the independent ADR rule from decision 3 still apply on that path.

Alternative: require OpenSpec for every change, or exempt all changes below a size threshold. Both substitute ceremony or diff size for actual uncertainty and risk. Letting agents insist on OpenSpec was also rejected: the user owns the workflow choice, and a direct change still meets every quality rule.

### 2. Give each document a distinct responsibility

- `AGENTS.md` defines how agents work and summarizes selection rules; it points to the human-facing explanation.
- Main capability specs under `openspec/specs/` describe maintained product behavior where specifications exist. Delta specs under a change describe proposed additions, modifications, or removals until synced. Neither tree is assumed to cover every existing feature.
- OpenSpec proposals, designs, and tasks describe an individual change's motivation, proposed approach, trade-offs, and implementation work. Designs may evolve while the change is developed; archived artifacts retain change context rather than becoming a current architecture manual.
- ADRs record accepted architectural decisions, their context, and consequences across changes.
- `docs/architecture.md` describes today's boundary and invariant map. README and manual QA documentation retain their existing usage and verification roles.

A proposal or design that conflicts with an accepted architectural decision must explicitly plan the required new or superseding ADR and current architecture updates. Creating or archiving the proposal is not itself architectural acceptance, supersession, or proof of implementation.

Alternative: use archived designs as a complete ADR replacement. That would scatter architectural commitments across change histories and leave direct changes without the established decision-record path.

### 3. Require ADRs for consequential architectural commitments, independently of OpenSpec

Retain the mandatory ADR rule for any change to an architecture invariant, including the existing public API constraint. Also use ADRs for new or revised architectural boundaries and durable policies that constrain later work, such as workspace-access policy or model-provider coupling. Routine algorithms, helper structure, or feature-local implementation choices remain in an OpenSpec design when one exists, or need no standalone decision document.

An OpenSpec change may need no ADR; a direct change may need one. When a decision fits an already accepted ADR without changing its commitment, reference that ADR rather than create another. Preserve historical immutability, numbering, and supersession rules. This workflow clarification itself does not change an architectural commitment and needs no new ADR.

Alternative: create one ADR per OpenSpec design or require OpenSpec whenever an ADR is warranted. Neither reflects the different purposes of architectural history and change planning.

### 4. Keep ADRs self-contained without copying implementation plans

When both an OpenSpec design and an ADR are needed, the ADR summarizes the accepted architectural choice, sufficient context to understand it, and its consequences. Keep detailed implementation alternatives, task breakdowns, and feature requirements in their own artifacts. Link from the design to relevant ADRs instead of repeating their full rationale.

The ADR must remain understandable on its own. Active change artifacts are not stable references: the change is not implemented yet, its proposal and design may still change, and its directory moves when archived. An ADR therefore never links an active change. Archived changes are implemented and no longer edited, so an ADR may link an archived change directory as supporting context, as long as it still makes sense without it. When an ADR is written while its change is still active, it may name the change, which keeps its name inside the dated archive directory, and the change's design links to the ADR. The ADR is not edited later to add an archive link, consistent with its immutability. Existing ADRs remain untouched by this change.

Alternative: copy the complete design into the ADR. This creates two accounts that are likely to diverge and makes architectural decisions harder to find.

### 5. Implement the guidance in the existing entry points

Add concise workflow-selection and document-ownership rules to `AGENTS.md`, preserving the existing implementation cycle and safeguards. Extend `docs/README.md` with the explanation, examples, and OpenSpec directory links, and refine the ADR authoring threshold while retaining its template and history rules.

Do not duplicate the policy in `openspec/config.yaml` or generated skills. Those files remain unchanged. Review the root README and architecture introduction for contradictions, but do not expand them merely to repeat this guidance.

Alternative: add a dedicated workflow manual or copy the full policy into OpenSpec configuration. Two existing entry points are sufficient for this small repository and avoid another source to maintain.

## Risks / Trade-offs

- Subjective OpenSpec threshold: mitigate with concrete examples and uncertainty/risk criteria rather than arbitrary size limits.
- Small-change exemption mistaken for a quality exemption: state explicitly that investigation, tests where applicable, verification, documentation maintenance, dependency approval, and scope rules apply on both paths.
- Existing specs drift after direct changes: require affected maintained specs to stay accurate without forcing a full change scaffold.
- Direct spec edits lost on sync: require an overlapping active delta to be updated together with the main spec.
- Agents pushing ceremony on the user: limit the recommendation to one mention with its reason and treat the user's choice of the direct path as final.
- Architectural policy hidden only in a design: keep the invariant-triggered ADR requirement explicit, regardless of change size or OpenSpec use.
- Duplicate or fragile rationale: make ADRs concise and self-contained, keep full change plans in OpenSpec, and allow ADR links only to archived changes.

## Migration Plan

Apply the documentation edits together with the amendment to the `add-content-search` ADR tasks. No code or data migration, retrospective ADR rewrite, other active-change rewrite, or capability-spec backfill is required. Verify the resulting guidance against four representative cases: a typo, a local contract-restoring bug fix, a nontrivial new capability, and a small architectural-invariant change. Also check a direct change to already-specified behavior, a direct edit to a requirement that an active delta also touches, a user who declines a recommended OpenSpec change, an ADR written while its change is still active, and an OpenSpec feature that introduces no architectural commitment.

Rollback consists of reverting the guidance edits; runtime behavior and architectural commitments remain unchanged in either direction.
