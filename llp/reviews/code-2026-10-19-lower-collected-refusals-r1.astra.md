# Code review: Lowering keeps collected refusals beside a fatal one, round 1, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `17d73adb17`.
- **Method:** one brief (sha256 `7b98f23c855367add657e47b0e02a69b9820f34420189432acb956644b9277ec`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one blocking (a needless borrow clippy -D warnings rejects), one nit (assert the exact pair and compile_path's first)
- **Disposition:** both taken in cc7055f272; clippy -D warnings clean on contract-lower and contract's tests

One blocking issue for the required lint gate; no functional regression found in the error aggregation.

1. **Blocking — redundant borrow after extraction.** At [lib.rs:674](/tmp/rv-fatal-1/contract/lower/src/lib.rs:674), `l` is now `&mut Lowerer`, so `host_transform_recipients(&l, view)` adds an unnecessary reference. This matches Clippy’s default-warning [`needless_borrow`](https://clippy.rust-lang.org/master/index.html#needless_borrow), which the required `-D warnings` rejects. Pass `l` directly.

2. **Nit — assert the fatal diagnostic explicitly.** The new assertions at [tamber.rs:822](/tmp/rv-fatal-1/contract/cli/tests/it/tamber.rs:822) check the first ID and count, allowing any second diagnostic. Assert the exact sequence `["tamber-under-light", "lower-share-picture"]`, and check `compile_path()` returns `tamber-under-light` for this mixed-error fixture. The existing singular-API assertion exercises the earlier collected-only case. The new cap and deduplication paths lack direct coverage.

The requested semantic checks otherwise look sound:

- Both `Err(Vec::new())` exits require nonempty `l.errors`; the outer function preserves their order and truncates to 20. Fatal deduplication uses the same ID/span/message comparison as `theme::push`.
- Successful builder operations remain unchanged. `_theme` and `_profiles` restore kernel parsing state; [finish()](/tmp/rv-fatal-1/plan/src/builder.rs:946) only interns strings and validates plan data. Neither finalization nor diagnostic-name rewriting needs those guards.
- No inspected caller requires the fatal ID first. `compile_path()` actually uses `lower_all()` followed by `first()`. CLI and design-server paths report diagnostics or display messages; Tamber’s synthetic re-fold checks success and retained metadata. The VS Code extension has no quick-fix implementation.
- A failed design fold can still cause a decoration’s valid token reference to produce a secondary `lower-attr-value`. That cascade predates HEAD; preserving its underlying design refusal first improves it. I would not block this change or broadly suppress fatal diagnostics for that case.

Read-only review of `17d73adb`; no builds, tests, or lint commands run.
