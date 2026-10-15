# Spec Delta

## Purpose

Let the agent locate text within workspace files without reading every candidate file individually, while retaining read-only access, bounded results, and clear search coverage.

## ADDED Requirements

### Requirement: Search workspace text

The system SHALL offer a `search_content` tool accepting a string `pattern`, optional workspace-relative `path`, optional `fixed_strings`, optional `case_sensitive`, and optional one-based `start_match`. An omitted path SHALL select the workspace root. A file target SHALL search that file; a directory target SHALL search recursively.

#### Scenario: Search without a path
- **WHEN** the agent searches for `needle` without a path
- **THEN** matching lines from eligible files throughout the workspace are returned

#### Scenario: Restrict the search to a file or subtree
- **WHEN** the agent supplies a file or directory path
- **THEN** only that file or eligible descendants of that directory contribute results

### Requirement: Explicit matching semantics

The system SHALL use case-sensitive regex matching by default. `fixed_strings: true` SHALL treat the pattern literally, and `case_sensitive: false` SHALL enable case-insensitive matching without smart-case heuristics. Searches SHALL operate on individual lines using standard Rust regex syntax, without PCRE2 extensions or semantic symbol resolution.

#### Scenario: Regex and literal searches differ
- **WHEN** a file contains `config.json` and `configXjson` on separate lines
- **THEN** regex pattern `config.json` matches both lines, while the same pattern with `fixed_strings: true` matches only the first

#### Scenario: Case sensitivity is explicit
- **WHEN** a file contains `Needle` and `needle` on separate lines
- **THEN** pattern `needle` matches only the second by default and both with `case_sensitive: false`

#### Scenario: Empty patterns retain grep semantics
- **WHEN** the pattern is an empty string
- **THEN** each existing text line matches, while an empty file contributes no matching lines

### Requirement: Actionable request errors

The system SHALL reject invalid regex syntax, unsupported regex constructs, and `start_match: 0` with model-actionable argument feedback. Literal mode SHALL accept regex metacharacters as ordinary text. Invalid requests SHALL NOT produce successful empty search results.

#### Scenario: Invalid regex
- **WHEN** a regex pattern contains an unclosed character class
- **THEN** the tool reports an argument error identifying the pattern problem without exposing host paths

#### Scenario: Invalid page offset
- **WHEN** `start_match` is zero
- **THEN** the tool reports that the offset must be at least one

### Requirement: Workspace containment and read-only execution

Every search target, discovered file, visited directory, and loaded ignore file SHALL remain within the canonical workspace boundary and undergo workspace checks before filesystem access. Search SHALL NOT write files, launch subprocesses, or require an installed search executable. Existing bounded regular-file reads SHALL remain enforced.

#### Scenario: Reject an explicit escape
- **WHEN** the requested path is absolute, escapes through `..`, or resolves through a symlink outside the workspace
- **THEN** the tool rejects it through the existing model-visible path-error policy and reads no content from that target

#### Scenario: An in-workspace explicit symlink target
- **WHEN** an explicitly named file is a symlink whose canonical target is a regular file inside the workspace
- **THEN** it can be searched after workspace validation

### Requirement: Filter recursive discovery

Recursive discovery SHALL exclude hidden entries and discovered symlinks, and SHALL apply workspace-local `.gitignore` and `.ignore` rules even outside a Git repository. It SHALL NOT consult ignore files above the workspace, global Git configuration, or `.git/info/exclude`. An explicitly named regular file SHALL bypass discovery filters, not workspace or read limits.

#### Scenario: Ignore generated and hidden entries
- **WHEN** a directory contains an ignored build file, a hidden file, a visible source file, and a symlink
- **THEN** only eligible visible non-symlink files contribute matches during recursive discovery

#### Scenario: Search an explicitly named excluded file
- **WHEN** an ignored or hidden regular file is explicitly selected
- **THEN** its text is searched if it passes workspace validation and read limits

#### Scenario: Restricting a subtree retains local ancestor rules
- **WHEN** the agent searches `src/` and the workspace-root `.gitignore` excludes `src/generated/`
- **THEN** that generated subtree remains excluded

#### Scenario: Machine-level rules do not change results
- **WHEN** a parent-directory ignore file or global Git ignore configuration excludes a visible workspace file
- **THEN** that external configuration does not affect search eligibility

### Requirement: Predictable local ignore precedence

The system SHALL interpret local ignore patterns with Git-style anchoring, negation, and directory rules. For each file type of ignore rules, deeper directories SHALL take precedence over ancestors. `.ignore` matches SHALL take precedence over `.gitignore` matches. Excluded directories SHALL be pruned; hidden-entry exclusion SHALL remain independent of ignore negation.

#### Scenario: Ignore files disagree
- **WHEN** `.gitignore` excludes a visible file and `.ignore` explicitly re-includes it
- **THEN** the file is eligible unless another independent discovery filter excludes it

#### Scenario: A nested rule overrides an ancestor
- **WHEN** a root `.gitignore` excludes `*.log` and a nested `.gitignore` re-includes `keep.log` inside a traversable directory
- **THEN** that nested `keep.log` is eligible

#### Scenario: A child rule cannot reopen a pruned parent
- **WHEN** a directory is excluded without being re-included
- **THEN** rules inside it are not loaded and its descendants are not searched

### Requirement: Stable matching-line results

Each matching text line SHALL appear once per searched file regardless of the number of occurrences on it, with a workspace-relative path, one-based line number, and matching line text. Unchanged inputs and filesystem contents SHALL yield deterministic file traversal order and ascending line order within each file. CRLF and LF input SHALL produce equivalent line numbering without a trailing carriage return in result text.

#### Scenario: Repeated occurrences on one line
- **WHEN** one line contains the search term three times
- **THEN** it contributes one result and advances the match offset by one

#### Scenario: Read surrounding context
- **WHEN** a result identifies `src/example.rs` at line 12
- **THEN** the same path and line number identify that line through `read_file`

### Requirement: Bounded stateless pagination

A page SHALL contain at most 200 matching lines and at most 65,536 UTF-8 bytes in total, including notices. `start_match` SHALL default to one and count matching lines across eligible files. When another matching line exists beyond a page, the response SHALL give the exact next `start_match`. Pages SHALL NOT require retained cursor state or claim snapshot consistency across edits.

#### Scenario: Retrieve later results
- **WHEN** unchanged files contain 201 matching lines and the first 200 fit the byte budget
- **THEN** the first page gives `start_match: 201` and that subsequent request returns the remaining line without duplication

#### Scenario: Exact page boundary
- **WHEN** there are exactly 200 matching lines and their output fits the byte budget
- **THEN** no more-results notice is returned

#### Scenario: The byte limit ends a page first
- **WHEN** the next complete result would exceed the page's byte budget
- **THEN** it is deferred to the next page and the continuation offset identifies that result

#### Scenario: Page beyond the result set
- **WHEN** `start_match` exceeds the number of matching lines
- **THEN** the tool explicitly reports that there are no results at or after that offset

### Requirement: Oversized results make progress

A matching line that cannot fit on a page by itself SHALL be represented by a UTF-8-safe clipped result with an explicit clipping notice. It SHALL count as one consumed matching line, and any continuation SHALL advance beyond it. The result SHALL preserve its file and line identity and SHALL NOT claim that pagination can recover clipped text.

#### Scenario: A very long matching line precedes another match
- **WHEN** the first matching line exceeds the output budget and a second matching line exists
- **THEN** the first page identifies the clipped first line and offers a continuation that reaches the second

### Requirement: Explicit supported-file coverage

Recursive search SHALL skip non-regular entries, files exceeding the existing 1 MiB read limit, non-UTF-8 files, and files containing NUL bytes. It SHALL report aggregate counts of oversized and unsupported-text files encountered, without raw host diagnostics. Files deliberately excluded by discovery filters SHALL NOT be presented as read failures. Coverage notices on paginated results SHALL describe only the traversal performed for that response.

#### Scenario: Unsupported files accompany matching text
- **WHEN** recursive search encounters an eligible text match, an oversized file, and a non-UTF-8 or NUL-containing file
- **THEN** it returns the match and an aggregate skipped-file notice without searching unsupported content

#### Scenario: No matches with skipped files
- **WHEN** no supported file matches but eligible unsupported files were skipped
- **THEN** the result reports no matches among searched text and the skipped-file notice rather than claiming complete text coverage

### Requirement: Preserve genuine failure visibility

Unexpected filesystem failures and invalid or unreadable applicable ignore files SHALL fail the search through host-only error handling rather than return successful partial results. Explicit file targets that are oversized or unsupported text SHALL also fail through host-only handling. Host paths and operating-system diagnostics SHALL remain unavailable in model feedback and available in operator diagnostics.

#### Scenario: Directory access fails after a match
- **WHEN** traversal encounters an unexpected I/O error after finding a match but before completing the current page's traversal
- **THEN** the call fails with generic model feedback and retains the detailed cause for the operator

#### Scenario: Applicable ignore file cannot be interpreted
- **WHEN** a local applicable ignore file is unreadable, exceeds the read limit, is not valid UTF-8, or contains an invalid pattern
- **THEN** the call fails rather than silently searching under different ignore rules

#### Scenario: Explicit unsupported file
- **WHEN** the requested file exceeds the read limit or is unsupported text
- **THEN** the call fails with generic model feedback instead of reporting zero matches

### Requirement: Existing tool-call visibility

The system SHALL report each `search_content` attempt through the existing pre-execution tool-call notification, including its string pattern and any explicitly supplied string path. Numeric offsets and boolean matching options SHALL NOT become notification targets. Adding this capability SHALL NOT widen the public library API or add UI-specific types to the core.

#### Scenario: Observe a search attempt
- **WHEN** the agent searches pattern `needle` in `src` with a later page offset and literal matching
- **THEN** the tool notification identifies the search name, path, and pattern before execution, while omitting numeric and boolean options
