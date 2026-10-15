# Tasks

## 1. Checked traversal and local filtering

- [ ] 1.1 Add compatible `grep-regex`, `grep-searcher`, and `ignore` dependencies and update `Cargo.lock`; verify the resolved versions support the pinned toolchain and the in-memory matching and line-fed ignore APIs described in the design.
- [ ] 1.2 Add focused workspace behavior tests before changing production behavior, then implement the private checked target-classification and directory-enumeration operations needed by search; verify file/directory targets, regular-file constraints, internal symlinks, absolute paths, parent escapes, and external symlinks preserve the current sandbox contract without changing existing tools.
- [ ] 1.3 Add the private `src/tools/search_content.rs` module and deterministic depth-first traversal using sorted directory entries and an explicit stack; verify repeatable cross-directory ordering, pruning of hidden entries, and non-following of file and directory symlinks with temporary-workspace tests.
- [ ] 1.4 Load regular local `.gitignore` and `.ignore` files through bounded workspace reads and feed their text to `ignore` matchers; verify anchoring, negation, nested precedence, `.ignore` priority, rules outside a Git repository, root-to-subtree ancestor rules, and pruned-directory behavior through observable file eligibility tests.
- [ ] 1.5 Verify explicit file targets bypass discovery filters while retaining read limits, explicitly selected directories form the traversal root, and parent/global rules do not affect results; test applicable malformed or unreadable ignore files through deterministic error cases without depending on host permissions.
- [ ] 1.6 After a real-filesystem traversal smoke run, add the next numbered ADR recording only the embedded-search and workspace-local filtering policy; verify it describes the checked access path and filtering policy, may name but does not link this change, leaves limits, clipping, and pagination to the delta spec and design, and neither edits historical ADRs nor changes architecture invariants.

## 2. Matching, bounded results, and pagination

- [ ] 2.1 Add behavior tests for regex versus literal matching, explicit case handling, invalid regexes, unsupported constructs, empty patterns, CRLF/LF input, and one result per matching line; implement one reusable matcher/searcher per call using only checked UTF-8 slices, and verify returned paths and line numbers identify the same content through `read_file`.
- [ ] 2.2 Implement recursive unsupported-file handling and explicit-target failures; verify oversized, non-UTF-8, and NUL-containing files never contribute matches, recursive skips produce aggregate coverage information, explicit unsupported files fail, and genuine I/O failures do not return successful partial results.
- [ ] 2.3 Add pagination boundary tests before implementing one-based `start_match`; verify multiple pages across files have neither gaps nor duplicates for unchanged content, zero is rejected, offsets beyond the end are explicit, and continuation is offered only after another matching line is found.
- [ ] 2.4 Implement the 200-line and 65,536-byte whole-response limits, including all notices; verify exact boundaries, a byte-limited page before the line limit, footer-induced row deferral, and UTF-8-safe clipping that preserves file/line identity and advances past an oversized result.
- [ ] 2.5 Exercise matching and a later page in a throwaway real-filesystem smoke run, then verify limits, clipping, rescan semantics, and page-local skipped-file counts against the delta spec and design rather than recording them in the ADR; verify the offered continuation reaches the expected next matching line and the output makes no snapshot or whole-tree coverage claim.

## 3. Rig exposure and user-facing documentation

- [ ] 3.1 Add the thin `search_content` Rig adapter with `pattern`, `path`, `fixed_strings`, `case_sensitive`, and `start_match`; reuse workspace error mapping and add explicit argument-versus-host error handling, verified by assertions that actionable errors remain useful while raw host paths and source diagnostics are redacted from model feedback.
- [ ] 3.2 Register `SearchContent` in `AgentSession::with_model` and update the existing exact read-only tool-set guard; verify the exposed tools are exactly `list_directory`, `read_file`, and `search_content`, while the existing five-type public API remains unchanged.
- [ ] 3.3 Exercise registered search against a real temporary workspace through the current Rig/model injection seam or a throwaway integration harness; verify actual tool-result contents, a continuation page, and a refused escape rather than relying on a canned model reply or a forwarding-only assertion.
- [ ] 3.4 After the registered-path smoke proof, update `README.md`, the request-path prose in `docs/architecture.md`, roadmap item 1, and `docs/testing.md`; verify they describe the delivered search interface, filtering, limits, pagination, unchanged read-only boundary, and a live search-to-`read_file` QA scenario. Check `AGENTS.md` for staleness but leave it unchanged if its current guidance remains accurate.

## 4. Integrated verification

- [ ] 4.1 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, and `cargo test --all-targets --all-features --locked` after integration; verify all relevant search behavior and existing public API/sandbox/history tests pass without suppressing findings or changing unrelated behavior.
- [ ] 4.2 Run the documented search, later-page, follow-up file-read, and escape scenarios through the real implementation. If a tool-calling Ollama model and terminal are available, observe the existing pre-execution tool notification and final reply in the TUI; otherwise report the unexercised visual boundary explicitly. Verify the workspace stays unmodified by tool execution and remove any smoke scaffolding created for verification.
