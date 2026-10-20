# Code review: contract tamber check/eval: a design's icon SVGs read under its app's assets/, round 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `3bfe4639e2`.
- **Method:** one brief (sha256 `d0f8f1d1ad64db23a47d335bedd4c25ed7c1491ac4b411150f96541d0127a902`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (assets beside a design app.json names elsewhere), one should-fix (Windows symlink), one nit (cleanup on failure)
- **Disposition:** all taken in 347daa2996

**Blocking — Assets are taken from the design file’s directory, not the app’s — `contract/cli/src/tamber.rs:174`**

`admission` is given `path.parent().join("assets")`, and `path` is the design file. The build reads icons from the app root (`app.contract`’s parent, `contract/cli/src/lib.rs:710`). With `app.json` `"tamber": "design/shelf.tamber"` (schema and `contract/cli/tests/it/tamber.rs:108`), `contract tamber check <app>` (and `eval` / `ir`) links `design/assets`. A real `assets/icons/reply.svg` is still refused; a different file under `design/assets` is accepted. Passing that `.tamber` path directly is the same wrong directory: `app_contract` only looks beside the file (`tamber.rs:423`), so `apart` is true and the link follows the design. Fix: link `built.parent().join("assets")` whenever the design belongs to the app. For a `.tamber` named from another directory, walk up to the `app.json` whose `"tamber"` is this file and use that directory.

**Should-fix — A Windows symlink failure fails the whole verb — `contract/cli/src/tamber.rs:433`**

`symlink_dir` needs Developer Mode or `SeCreateSymbolicLinkPrivilege`. Any no-params app that has an `assets/` directory hits `link_dir` (`tamber.rs:286`), and that error becomes exit 2 (`tamber.rs:290`) even when the design has no icons. There is no fallback. On Windows, create a directory junction (unprivileged). Rust 1.97 `remove_dir_all` opens reparse points with `FILE_FLAG_OPEN_REPARSE_POINT` and does not walk them, so either a symlink or a junction is removed without deleting the target. A copy is the other safe fallback.

**Nit — The error return skips cleanup — `contract/cli/src/tamber.rs:290`**

`remove_dir_all` runs only after a successful setup (`tamber.rs:294`). A failed `link_dir` leaves `contract-tamber-<pid>` behind. Remove the directory on that path too (a drop guard covers it).

Beside the app, relative and absolute paths are right. `app.tamber` from the app directory, `../app/app.tamber` from elsewhere, and an absolute path all keep the design’s directory, and `canonicalize` (`tamber.rs:283`) stores an absolute target so the link is not resolved against the temp dir. A symlinked `assets/` is followed to the same directory the build reads. An `svg` path cannot contain `..` (`tamber/src/icon.rs:82`). Cleanup of that symlink does not delete the target. The temp directory is `contract-tamber-<pid>` and `admission` runs once per process, so concurrent CLIs do not share it.

The new test (`tamber_icons.rs:585`) covers an absolute path from another cwd, `app.tamber` and `.` from the app directory, `check` and `eval`, and a missing SVG. It never uses a `"tamber"` path in another directory, a relative path from a third cwd, or a symlinked `assets/`, and it does not check that the real SVG is still there after the process exits. The copy still has no `app.json` on purpose, so once the file reads, per-target refusals (non-template `tamber-icon-svg`, `tamber-icon-missing`) still do not run.
