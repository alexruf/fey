# Proposal

## Why

OpenSpec now provides change proposals and design rationale, while the existing project guidance independently asks for plans and ADRs without explaining their boundaries. Contributors need one clear policy that avoids duplicate documentation and lets small, well-understood changes proceed without OpenSpec overhead.

## What Changes

- Clarify in `AGENTS.md` and `docs/README.md` when a change benefits from OpenSpec and when direct implementation is appropriate. Do not require OpenSpec artifacts for every change. Agents may recommend OpenSpec but never require it: the user decides, and a choice of the direct path is final.
- Separate behavioral requirements in capability specs, change-specific planning and design in OpenSpec change artifacts, accepted architectural rationale in ADRs, and the current architectural boundary and invariants in `docs/architecture.md`.
- Define the ADR threshold independently of whether OpenSpec is used: record consequential architectural commitments, and always write an ADR for changes to architecture invariants. Do not require an ADR for routine implementation decisions.
- Explain how a concise, self-contained ADR can refer to related change context without duplicating an entire OpenSpec design. ADRs never link active changes, whose plans are unimplemented and may still change; they may link archived changes, which are implemented and stable.
- Preserve testing, verification, documentation maintenance, dependency approval, and scope rules for both the OpenSpec and direct paths. Small changes must still keep any affected existing capability specs accurate, including any active change's delta spec for the same requirement, so that a later sync does not discard the direct edit.
- State that an OpenSpec proposal or design does not supersede an accepted ADR by itself, and that historical ADRs retain their existing immutability and supersession rules.

## Capabilities

### New Capabilities

None. This is contributor-workflow documentation, not a new runtime capability. This change declares `skip_specs: true`; it does not create artificial product requirements for process guidance.

### Modified Capabilities

None. No existing product behavior or capability requirements change.

## Impact

- Update `AGENTS.md` with terse agent-facing workflow selection and document-responsibility rules.
- Update `docs/README.md` with human-facing explanations, decision criteria, examples, and links to OpenSpec's main specs and change directories.
- Check `README.md` and `docs/architecture.md` for contradictions; leave them unchanged if their current content remains accurate. No new ADR is required because this clarifies documentation responsibilities without changing an architectural commitment or invariant.
- Leave `openspec/config.yaml`, generated workflow skills, and historical ADRs unchanged. Do not backfill specs for existing features or require retrospective changes to existing plans.
- Amend `add-content-search` tasks 1.6 and 2.5 so its planned ADR records only the embedded-search and local-filtering policy, while limits, clipping, rescan semantics, and skipped-file counts stay in its spec, design, and user-facing documentation. Without this, the active plan would contradict the new ADR guidance as soon as it lands.
- No Rust code, dependencies, public APIs, runtime behavior, or quality-gate changes.
