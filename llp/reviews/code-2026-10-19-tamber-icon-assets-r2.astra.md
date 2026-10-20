# Code review: contract tamber check/eval: a design's icon SVGs read under its app's assets/, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `347daa2996`.
- **Method:** one brief (sha256 `a3d556b7375196f64ac087fe8145f1b4b349df2811a67f9d37898b87e7f22502`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** no blocking; two should-fix (a design checked by its own path under an app above it; a read-only SVG copied twice), one nit (test bytes)
- **Disposition:** all taken in d4ea9922af

No blocking findings; two should-fix issues remain.

- **Should-fix — [tamber.rs:174](/tmp/rv-icons2/contract/cli/src/tamber.rs:174):** With `"tamber": "design/theme.tamber"`, directly checking `design/theme.tamber` still selects `design/assets`: `app_contract()` assumes the app is beside the design and sets `apart=true`. Checking the app directory works. **Fix:** resolve a matching ancestor manifest before deciding `apart`, starting from an absolute path. Add relative/absolute direct-file cases; [the new test:617](/tmp/rv-icons2/contract/cli/tests/it/tamber_icons.rs:617) only exercises directory and `app.contract` inputs.

- **Should-fix — [tamber.rs:303](/tmp/rv-icons2/contract/cli/src/tamber.rs:303):** Two icons or aliases sharing a read-only SVG copy it twice. [`fs::copy` preserves permissions](https://doc.rust-lang.org/std/fs/fn.copy.html), so the second copy can fail; Windows explicitly [refuses overwriting read-only destinations](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-copyfileexw). **Fix:** deduplicate SVG paths and write writable temporary copies. Add a read-only SVG shared by an icon and its alias.

- **Nit — [tamber_icons.rs:634](/tmp/rv-icons2/contract/cli/tests/it/tamber_icons.rs:634):** `is_file()` proves existence, not unchanged contents or cleanup. **Fix:** test symlinked assets and compare source bytes after success and failure; assert temporary-directory removal.

Otherwise, ordinary relative/absolute paths look correct. Copies remove the Windows symlink-privilege requirement and leave source targets untouched during cleanup. Source symlinks can still escape the app, matching existing compiler behavior. Separate CLI processes have distinct PID directories.

Static review only; no files modified, builds, or tests run.
