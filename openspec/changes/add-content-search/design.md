# Design

## Context

See `proposal.md` for motivation and `specs/content-search/spec.md` for the behavior contract.

The existing tools separate plain filesystem behavior (`src/tools/read_file.rs` and `list_directory.rs`) from Rig adapters (`src/tools/mod.rs`). `Workspace` constructs the private `ResolvedPath` capability and reads regular UTF-8 files with a 1 MiB limit, rechecked on the opened handle. `map_workspace_error` separates model-actionable path failures from host-only diagnostics. Search must extend these paths rather than create an independent filesystem client.

`AgentSession::with_model` registers the two existing tools. Its exact-tool-set test guards read-only exposure. Tool-call notifications already select string arguments in argument-name order, so a pattern and optional path need no new public type or TUI protocol.

The current project has no main OpenSpec specifications. This change adds one cohesive `content-search` capability and preserves architecture invariants I1-I8.

## Goals / Non-Goals

**Goals:**

- Reuse search and ignore-pattern implementations while retaining control over every filesystem read.
- Process files one at a time, compile the pattern once per call, and avoid collecting the whole tree or all matches.
- Make pagination progress and coverage understandable without server-side cursors.
- Keep new types private to the core and the Rig adapter thin.

**Non-Goals:**

- Exact parity with every ripgrep CLI default or flag.
- A general filesystem framework, changes to the existing tools' behavior, asynchronous parallel traversal, or a search index.
- A new hardening scheme for concurrent hostile filesystem mutation. Preserve the current canonical-path checks and bounded opened-file reads without claiming race-free OS-level isolation.

## Decisions

### 1. Embed three focused ripgrep libraries

Use `grep-regex` for pattern construction, `grep-searcher` for line-oriented matching and line numbers, and `ignore` for Git-style ignore matching. Add compatible versions to `Cargo.toml` and update `Cargo.lock` during implementation; the exploration approved embedded dependencies, not a runtime executable requirement.

Build one `RegexMatcher` and one reusable `Searcher` per tool call. Configure fixed-string and case options explicitly, disable smart case and multiline searching, and use `Searcher::search_slice` on bytes already returned by `Workspace::read_utf8_file`. Do not call `search_path`, invoke `rg`, use memory-mapped file access, transcode input, or enable PCRE2. Disable BOM transcoding so the content and line identity remain aligned with `read_file`. Normalize line terminators for display without trimming ordinary whitespace.

Alternative: launching `rg` would reduce search integration code but add process lifecycle, executable availability, configuration control, and a second filesystem-access path. A handwritten matching engine would duplicate established library behavior. Using only a regex crate is possible, but the embedded line searcher already owns match reporting and line numbering.

Verified API references:

- <https://docs.rs/grep-regex/latest/grep_regex/struct.RegexMatcherBuilder.html>
- <https://docs.rs/grep-searcher/latest/grep_searcher/struct.Searcher.html>
- <https://docs.rs/grep-searcher/latest/grep_searcher/struct.SearcherBuilder.html>
- <https://docs.rs/ignore/latest/ignore/gitignore/struct.GitignoreBuilder.html>

These references establish API shape, not a requirement to use the latest published versions regardless of toolchain compatibility. Check the selected versions' actual APIs during implementation.

### 2. Add one private helper and one Rig adapter

Implement search behavior in `src/tools/search_content.rs`, following the existing tool modules. Add a private search error type for invalid patterns, zero offsets, unsupported explicit text, ignore-rule errors, and wrapped workspace failures. Keep result-building and traversal state local to a call.

The adapter exposes:

| Argument | Type | Default |
| --- | --- | --- |
| `pattern` | string, required | none |
| `path` | optional string | workspace root |
| `fixed_strings` | optional boolean | false |
| `case_sensitive` | optional boolean | true |
| `start_match` | optional positive integer | 1 |

An empty pattern keeps normal grep semantics and matches existing lines. No separate empty-pattern rejection is needed. `start_match` counts matching lines, not regex occurrences. Invalid regex syntax, unsupported constructs, and zero offsets are argument errors before file reads.

Register the generated `SearchContent` tool alongside `ListDirectory` and `ReadFile`. The existing notification renders its string arguments in argument-name order: path, when supplied, followed by pattern. Do not change `ToolCall`, the public export gate, or TUI rendering to achieve a preferred argument display order.

### 3. Keep target resolution and traversal behind workspace checks

Add only the private workspace operations needed to classify a checked target, enumerate a checked directory with entry kinds, and construct workspace-relative child paths. Reuse `resolve_file`, `resolve_dir`, and `read_utf8_file`; do not change existing tools merely to make them share a new abstraction.

A directory search uses a deterministic depth-first traversal. Sort each directory's entries by their native name ordering before processing them; process each file's lines in ascending order. Maintain an explicit stack so traversal depth does not consume the Rust call stack. This establishes repeatable file traversal order, not a promise of one globally lexicographically sorted list of all paths.

Do not follow discovered symlinks, including symlinked directories, special entries, or symlinked ignore files. Explicit file targets may resolve through an internal symlink, using the existing canonical containment check. Display the normalized workspace-relative requested/discovered path, never an absolute canonical host path. A non-round-trippable discovered filename must produce a host-only diagnostic rather than a lossy path that cannot identify the result through `read_file`.

Revalidate every entered directory and searched file. Skipping symlinks alone is not the containment check. File reads still enforce regular-file kind and the opened-handle size bound.

### 4. Use local ignore matchers, not an autonomous filesystem walker

`ignore::WalkBuilder` reads ignore files itself and defaults to parent/global configuration. Disabling parent/global options would not make its remaining reads pass through `Workspace`. Instead, use `ignore::gitignore::GitignoreBuilder` as a parser and matcher over already checked, bounded rule text.

Before enumerating a directory, load its regular non-symlink `.gitignore` and `.ignore` files through `Workspace`. Feed their lines through `add_line`, preserving each rule file's directory as its matching base. Do not use `GitignoreBuilder::add`, `build_global`, or another helper that opens a path internally. Missing rule files are normal; read or parse errors are not.

Maintain the active ancestor matchers on the traversal stack. Evaluate innermost-to-outermost `.ignore` matchers first, then `.gitignore` matchers if no `.ignore` rule matched. Within a rule file, use the library's last-match and negation semantics. Do not implement glob syntax ourselves. An excluded directory is pruned before its rule files are loaded. Hidden entries stay excluded independently of whitelist matches.

For a subtree target, preload applicable rule files from the workspace root through the target's ancestors, stopping at the workspace boundary. The explicitly selected directory is the traversal root even if an ancestor rule would omit discovery of that directory; those ancestor rules still govern its descendants. An explicitly selected file bypasses ignore loading and discovery filters entirely. Do not consult global configuration, ancestor directories outside the workspace, `.git/info/exclude`, or `.rgignore`.

This deliberately provides the agreed local filtering rather than full ripgrep CLI compatibility. No hidden/ignored override flags are added in this change.

### 5. Distinguish exclusions, unsupported content, and real failures

Deliberately filtered entries and non-regular entries are not attempted text reads and do not generate warning floods. During recursive discovery, oversized files, invalid UTF-8, and NUL-containing content are excluded from matching with aggregate counters for oversized and unsupported-text files. Detect a NUL before handing the slice to the searcher so a binary file cannot contribute early matches before being identified as binary.

Only those known content exclusions are successful skips. Unexpected metadata, traversal, read, or ignore-parse errors fail the call. Do not silently return the matches accumulated before such an error. For explicitly selected files, oversized or unsupported content is a host-only failure, consistent with `read_file`.

Reuse `map_workspace_error` for workspace failures. New host-only search failures use `ToolExecutionError::from_error` with detailed source chains; pattern/offset mistakes use `invalid_args`. Do not embed raw operating-system or ignore-file diagnostics into successful tool results. Aggregate counters are stable coverage information, not raw errors.

A page stops after it can prove a continuation exists. Its counters therefore cover only the part traversed for that call, including the offset skipped and the lookahead needed. They do not claim a whole-tree file inventory, and repeated pages do not provide cumulative counts.

### 6. Stateless pagination with a complete-response byte bound

Adopt the proposed defaults: 200 matching lines per page and 65,536 UTF-8 bytes for the entire response, including separators and notices. Treat these as private constants, not extra model arguments.

For each matching line, advance the global matching-line count. Ignore rows before `start_match` without formatting or copying them. Format emitted rows as `path:line: text` into a bounded response buffer and retain row boundaries needed for final packing. Do not retain all matches or perform a preliminary count of the whole tree.

Look ahead until another matching line is found or eligible input is exhausted; encountering another file or another nonmatching line is not evidence of more results. The exact next offset is the requested offset plus the number of emitted rows. Never promise an exact remaining count, since it would require continuing the whole search.

Account for actual footer text before finalizing the response. If adding continuation, clipping, or skipped-file notices would exceed the byte budget, defer complete trailing rows and adjust the next offset accordingly. Reserve room before clipping an oversized first row, preserving path/line identity and UTF-8 boundaries. A clipped row is consumed once, explicitly says its remaining text is not recoverable through `start_match`, and cannot trap the caller on the same result forever. Do not claim clipping when a complete row can fit on its own page.

No-match and beyond-end results are distinct. If unsupported files were encountered, qualify the no-match result as applying to searched text and include the coverage notice. If a page is truncated, require the same path, pattern, and matching options for continuation.

Alternative: opaque cursors would avoid rescanning but introduce mutable session state, expiry, and snapshot questions. Requiring narrower searches alone would depart from the requested pagination. Offsets fit the existing stateless tools.

### 7. Bound per-file work without adding a new global cutoff

Read at most one bounded content file at a time and stop once page completion plus lookahead is established. Compile the regex once, reuse the searcher, and maintain only active traversal/ignore state and the bounded output. Directory sorting retains the entries needed for deterministic order, as the existing listing tool does.

Do not introduce a wall-clock timeout, total-byte scan ceiling, or fixed file-count ceiling in this change. A no-match query may traverse the full eligible tree. Adding a new hard cutoff would require a separate continuation/coverage contract and could make matching files permanently unreachable through `start_match`. Narrowing the optional path is the existing way to restrict work.

### 8. Verify behavior without widening the presentation boundary

Use temporary workspaces and the current colocated unit-test style. Add permanent tests for meaningful matching boundaries, deterministic cross-file pagination, exact and byte-limited page endings, clipping progress, ignore precedence, explicit targets, symlink escapes, and error redaction. Use deterministic test seams for I/O failure behavior instead of permission-bit assumptions that vary under privileged execution.

Update the existing exact read-only tool-set assertion. Exercise the registered search against a real temporary filesystem and inspect returned search content, rather than treating a canned completion-model reply as evidence that search worked. Reuse the existing generic notification behavior; do not add tests that only echo forwarded arguments or re-pin presentation wording.

During apply, prove the new path with a throwaway real-filesystem smoke scenario including a later page and an escape attempt. If a live Ollama model and terminal are available, also observe the search notification and a follow-up `read_file` in the actual TUI. Report that visual limit if unavailable. Run formatting, Clippy, and the existing full test suite once after implementation is integrated.

## Risks / Trade-offs

- Rescanning for later pages costs repeated I/O. Mitigation: stop after lookahead, avoid formatting skipped matches, allow narrow paths, and document offset semantics.
- Files can change between requests, shifting offsets or coverage. Mitigation: document that pagination is not a filesystem snapshot and recommend restarting after edits.
- A no-match search can traverse a large eligible tree; per-file limits do not bound total time. Mitigation: local ignore rules, hidden-entry filtering, scoped paths, and no claim of a global work bound.
- An unreadable or malformed ignore file blocks that call instead of returning partial data. Mitigation: preserve detailed operator diagnostics and never silently change exclusion rules.
- Canonical-path validation has the pre-existing filesystem race window. Mitigation: retain current validation and opened-file checks, skip discovered symlinks, and do not claim stronger confinement than the existing workspace model.
- Case and filename ordering can vary by platform. Mitigation: deterministic ordering is guaranteed for unchanged inputs on the same platform, not identical cross-platform traversal of all possible filenames.

## Migration Plan

There is no persisted search state or public API migration. During apply, add compatible dependencies, implement checked behavior and its adapter, register the tool, and update the exact-tool-set guard. Document the delivered behavior in the README, architecture request path, roadmap, and manual QA playbook; add a new ADR for embedded search and local filtering without rewriting ADR-0004 or ADR-0005.

Rollback removes the search registration, its private helper/adapter, dependencies used only by search, and the corresponding new behavior documentation. Existing directory listing and file reading remain unchanged. This planning workflow makes none of those implementation edits.
