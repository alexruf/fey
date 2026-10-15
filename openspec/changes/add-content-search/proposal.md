# Proposal

## Why

The agent can list directories and read known files, but finding text across a project currently requires reading candidate files individually. A dedicated content-search tool supplies the third read-only capability identified in the roadmap without introducing shell access or semantic navigation.

## What Changes

- Add a `search_content` tool for line-oriented regex searches within a workspace-relative file or directory, defaulting to the workspace root.
- Support explicit literal matching and case-insensitive matching; retain case-sensitive regex matching by default.
- Return workspace-relative paths, one-based line numbers, and matching text, with deterministic ordering and stateless pagination.
- Respect workspace-local `.gitignore` and `.ignore` rules during recursive discovery, skip hidden entries, and do not follow discovered symlinks. Explicit file targets bypass discovery filters but retain sandbox checks.
- Reuse embedded ripgrep libraries for matching, line searching, and ignore-rule interpretation. Do not require an installed `rg` executable or launch subprocesses.
- Preserve bounded workspace reads and model/operator error separation. Distinguish deliberately excluded files, unsupported text files, and searches interrupted by real failures.
- Keep filename search, LSP operations, shell tools, PCRE2, multiline matching, context-output modes, and filter-override options out of scope.

## Capabilities

### New Capabilities

- `content-search`: Sandboxed, filtered, paginated searches of text inside workspace files.

### Modified Capabilities

None. The project currently has no main OpenSpec capability specifications.

## Impact

- Add filesystem/search behavior under `src/tools/`, a thin Rig adapter in `src/tools/mod.rs`, and registration in `src/agent.rs`.
- Extend private workspace helpers only where checked target classification, directory enumeration, and bounded ignore-file loading require them. Preserve the five-type public library API and existing sandbox invariants.
- Add direct Cargo dependencies on `grep-regex`, `grep-searcher`, and `ignore`, with corresponding lockfile updates during implementation. Use versions compatible with the pinned Rust toolchain.
- Extend behavior tests for search semantics, pagination, ignore precedence, sandbox escapes, and error visibility; update the existing exact read-only tool-set guard.
- Update the README, architecture request-path description, roadmap, and manual QA playbook when implementation lands. Record the embedded search and local ignore policy in a new ADR without rewriting historical ADRs.
- No TUI protocol or rendering change is required: existing tool-call notifications already display string arguments.
