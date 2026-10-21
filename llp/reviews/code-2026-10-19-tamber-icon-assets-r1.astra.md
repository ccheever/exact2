# Code review: contract tamber check/eval: a design's icon SVGs read under its app's assets/, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `3bfe4639e2`.
- **Method:** one brief (sha256 `d0f8f1d1ad64db23a47d335bedd4c25ed7c1491ac4b411150f96541d0127a902`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (Windows symlink privilege, no fallback), two should-fix (assets following the design, not its app; links escaping the app, pre-existing)
- **Disposition:** taken in 347daa2996: the named SVGs copied, no link; the app's assets for a design app.json names; the escape is the build's own, left

- **Blocking — [tamber.rs:434](/tmp/rv-icons1/contract/cli/src/tamber.rs:434): Windows has no fallback.** `symlink_dir` can fail without Developer Mode or symlink privileges. Because linking is unconditional whenever `assets/` exists, even designs **without icons** now fail admission. Provide a nonprivileged fallback or pass the original asset root into compilation; test denied symlink creation. [Rust documentation](https://doc.rust-lang.org/std/os/windows/fs/fn.symlink_dir.html).

- **Should-fix — [tamber.rs:174](/tmp/rv-icons1/contract/cli/src/tamber.rs:174): asset root follows the design, not its app.** With `"tamber":"design/theme.tamber"`, checking the app reads `design/assets/` instead of the app’s `assets/`. It can reject valid icons or validate the wrong files. Derive assets from `built.parent()` for app inputs, retaining design-relative behavior for standalone files. Add relative and absolute manifest-path cases.

- **Should-fix, pre-existing safety gap — [icons.rs:104](/tmp/rv-icons1/contract/lower/src/icons.rs:104): symlinks can escape the app.** Canonicalizing `assets/` resolves links but imposes no containment check; nested SVG links also follow external targets. Validate resolved SVG paths against the original app boundary in the shared reader. This behavior already exists in normal compilation.

Ordinary relative/absolute paths and cwd handling look correct for colocated designs. A symlinked `assets/` resolves correctly. Cleanup removes the link without deleting its target’s contents, per [`remove_dir_all`](https://doc.rust-lang.org/std/fs/fn.remove_dir_all.html). Separate concurrent CLI processes have distinct PID directories; no concurrent same-process admission caller exists.

The [new test](/tmp/rv-icons1/contract/cli/tests/it/tamber_icons.rs:585) covers both verbs, basic path/cwd combinations, missing SVG rejection, and indirectly target preservation through repeated calls. It lacks manifest overrides, symlink cases, Windows privilege failure, and concurrent invocations.

Static review only; no files modified, builds or tests run.
