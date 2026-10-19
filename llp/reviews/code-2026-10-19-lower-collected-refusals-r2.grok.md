# Code review: Lowering keeps collected refusals beside a fatal one, round 2, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via the Grok CLI, reasoning effort xhigh, no subagents, edits denied, a throwaway worktree at `cc7055f272`.
- **Method:** one brief (sha256 `30de7ef9a0aedbf531b22a23bbd20660892ee6b3700790327b32639329c7bd22`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking
- **Disposition:** none needed

Nothing blocking. Both round-1 fixes match the code.

1. **Borrow.** `host_transform_recipients` takes `&Lowerer`. In `lower_body`, `l` is already `&mut Lowerer`, so passing `l` is the reborrow. The old `&l` was `&&mut Lowerer` (it still coerced, and `clippy -D warnings` rejects it). Nothing else in `lower_body` takes `&l`, `&mut l`, or `l` by value. Method and field uses autoderef, including `&l.types.shapes`, which is a field borrow either way. The two old `return Err(l.errors)` moves are now `Err(Vec::new())`, and the caller `mem::take`s `l.errors`.

2. **Test.** A light-side mismatch is pushed during `declare_theme` (`tamber-under-light`) and lowering continues with the axes still bound, so `setMode` does not add `tamber-mode`. `share(files=["#missing"])` then returns `lower-share-picture` through `?` before the view. The merge is collected, then fatal: exactly `["tamber-under-light", "lower-share-picture"]`. `compile_path` uses `swap_remove(0)` after `shown_errors`, which does not reorder when there are no params, so the first id is `tamber-under-light`.

The outer comment matches that order. `lower_body`'s comment matches the split: its own refusal is the `Err` (empty at the two collected-only stops), and collected refusals stay in `l.errors` for the caller to deduplicate and truncate.
