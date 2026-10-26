# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 3 of 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `3016f733c7`.
- **Method:** one brief (sha256 `110832621ea1b7739a418e7e6ea7594e695d50c85d07e714784fcbaf0727be7b`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (0 BLOCKER, 2 SHOULD-FIX, 1 NIT mentions).
- **Disposition:** taken (the round 3 commit).

- **SHOULD-FIX — Top-only inset removal can leave stale route padding.** [cover.rs:99](kernel/src/kernel/cover.rs:99). With `Edges → wrapper → inset scroller`, let the wrapper retain `Inset([20, 0])` while changing its overflow to visible. Clearing that inset changes the traversal from stopping at the wrapper to reaching the scroller, but none of the invalidation predicates changes. The route retains its padding until another restyle. **Fix:** include changes in `Inset` presence in the predicate, still invalidating only the nearest eligible `Edges` ancestor. Test both reporting orders.

- **SHOULD-FIX — Absolute children still miss changes to the header’s contribution to the top cover.** [cover.rs:347](kernel/src/kernel/cover.rs:347), [environment.rs:163](kernel/src/kernel/environment.rs:163). Changing a replaced header’s `padding-top`, toggling its `Whole` cover, or changing the safe-area inset restyles the route without restyling its absolute children. A `top:0` child can retain a 106-point shift after the correct shift becomes 44. **Fix:** restyle the route’s absolute children whenever `header_inset` changes, including environment updates. This gap predates these commits but falls within the requested staleness audit.

- **NIT — The unmirrored-walk fix is incomplete.** [cover.rs:358](kernel/src/kernel/cover.rs:358). Display, position, and overflow changes still call `covering()` before checking for an engine node, adding an unnecessary ancestor walk on web/wasm. **Fix:** guard `chain_changed` before traversal when no layout tree exists; likewise guard `set_host_cover`’s upward search.

The four named round-2 cases are addressed in code and regression tests; Q1 and Q2’s ancestor boundary are preserved. I found no additional host-specific regression: Android shares Linux’s cover path; terminal reports no covers.

Static review only: the read-only checkout has no build artifacts, so tests and performance measurements were not run.
