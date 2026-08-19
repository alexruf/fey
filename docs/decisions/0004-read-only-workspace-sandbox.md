# 0004. The model can only ever read inside the project folder

Status: Accepted
Date: 2026-08-17

## Context

`fey`'s first version gives the model tools to look at files, but no ability to write or run
anything. The paths it uses to do that come from the model itself, as plain text — and a model
can construct a path like `../../etc/passwd`, an absolute path, or point at a symlink that leads
somewhere outside the project, whether by mistake or because it was tricked into it. Whatever
protects against that has to hold up no matter which of those tricks is used.

## Decision

When `fey` starts, it resolves the project folder to its one true, canonical location on disk.
From then on, every path the model asks for gets resolved the same way — joined onto that
location, resolved to its real canonical path (which also resolves through any symlinks along the
way), and then checked that it still lands inside the project folder. `..`, absolute paths, and
symlinks that point outside are all caught by this same check, rather than trying to catch each
trick individually by inspecting the text of the path.

Reading is also capped: listing a folder stops at 200 entries, and reading a file stops at 200
lines or 64 KB, whichever comes first. Both checks look one step past the limit first, so the
"there's more, but it was cut off" message only ever shows up when something was actually cut off
— never as a false alarm. Reading a file also never cuts a line in the middle of a multi-byte
character.

If the folder `fey` was started in can't be found or isn't actually a folder, it refuses to start
at all — it never quietly falls back to some other folder like your home directory.

## Consequences

- Any new tool that touches the filesystem has to go through this same path-resolving step —
  never read a model-supplied path directly off the disk.
- This approach automatically handles symlinks correctly, because it resolves the real path first
  and checks it afterward, rather than trying to special-case symlinks. A new way to escape the
  sandbox would have to defeat that resolution step itself, not just avoid an explicit check.
- Giving the model the ability to write files or run commands is intentionally out of scope here.
  That needs its own decision about how a user would approve something risky before it happens —
  see [the roadmap](../roadmap.md).
