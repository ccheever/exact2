# Code review: contract tamber check/eval: a design's icon SVGs read under its app's assets/, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `347daa2996`.
- **Method:** one brief (sha256 `a3d556b7375196f64ac087fe8145f1b4b349df2811a67f9d37898b87e7f22502`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking; one should-fix (the same design-by-path case)
- **Disposition:** taken in d4ea9922af

**Should-fix:** `contract tamber check|eval|ir` on the design file `app.json` names still reads `assets/` beside that file — `contract/cli/src/tamber.rs:174`

`assets` is `built.parent()` only when `apart` is false. For a `.tamber` path, `app_contract` (`tamber.rs:441`) joins `app.contract` onto that file's parent, so `design_path` misses an ancestor app and `apart` is true (`tamber.rs:94`). Assets then come from `path.parent()` (`tamber.rs:175`). `contract tamber check /app/design/theme.tamber` (or `design/theme.tamber` from the app directory) looks for `/app/design/assets/…` and refuses every icon SVG, while the same design checked as the app directory or `app.contract` works. `tamber gen` already treats that file as the app's design by walking ancestors (`contract/cli/src/tamber_gen.rs:544`).

Fix: for a `.tamber` input, walk ancestors the same way and use the directory whose `app.contract` exists and whose `design()` canonicalizes to this file; use that directory's `assets/`. If none matches, keep the design file's directory (checked apart). Extend `the_tamber_verbs_read_icon_svgs_under_the_apps_assets` (`contract/cli/tests/it/tamber_icons.rs:617`) to run `check` and `eval` on `design/theme.tamber` itself, absolute and relative.

Checked and not bugs:

- Relative and absolute app dir, `app.contract`, and a beside-file `app.tamber` resolve `assets/` against the process cwd. `Path::new("app.tamber").parent()` is `""`, and `"".join("assets")` is `assets`.
- SVG names cannot leave `assets/`: `tamber/src/icon.rs:78` rejects `..`, `.`, empty segments, a leading `/`, `\`, and `:`. The copy destination stays under the temp dir.
- A symlinked `assets/` (or SVG) is followed by `is_file` and `std::fs::copy`, which writes a normal file. `remove_dir_all` on the temp dir (`tamber.rs:308` and `:312`) deletes those copies and does not remove the link target. No `symlink` / `symlink_dir`, so no Windows privilege and no fallback.
- The temp dir is `contract-tamber-{pid}` (`tamber.rs:281`). Each CLI process admits once; parallel processes have distinct pids.

Residual: the synthetic app still has no `app.json` (`tamber.rs:286`), so `targets()` is empty. A multi-colour SVG the real build refuses for a shipped host is now admitted, because the file is found and the target loop does not run. Copying the whole manifest is unsafe (`strings.base`, widgets). Same hole already existed for an SF-only icon (`tamber-icon-missing`).
