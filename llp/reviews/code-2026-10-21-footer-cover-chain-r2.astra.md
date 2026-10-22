# Code review: an absolute footer over a route's bottom-edge scroller rests on the tab bar (kernel cover chain), round 2 of 3, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `201cb1b7f6`.
- **Method:** one brief (sha256 `85cb1af022b13fe692d41543717f224def904fe6dcbc53de668d53eff388dac3`), blind to the other reviewer; 3 rounds (kernel, theme2's plan approval) then a narrow check. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** changes (2 BLOCKER, 2 SHOULD-FIX, 1 NIT mentions).
- **Disposition:** taken (the round 2 commit).

- **BLOCKER — [cover.rs:98](kernel/src/kernel/cover.rs:98): `Edges` boundary changes leave ancestor padding stale.** For `sheet → route → scroller`, set the sheet’s `Edges`, then the scroller’s bottom `Inset`, then the route’s `Edges`. The sheet loses its padding during the second call and never regains it. Clearing the inner `Edges` has the reverse problem. Include entering/leaving `Edges` in ancestor invalidation, and test all cover-order permutations.

- **BLOCKER — [txn.rs:921](kernel/src/txn.rs:921): a footer attached after covers are installed still sits under the bar.** `CreateView → SetStyle(position:absolute, bottom:0) → SetChildren` derives the footer before it has a covered parent. Attachment re-derives the parent, but not the footer; repeating an unchanged cover returns early. Re-derive absolute children whose parent changes, including detachments. Test insertion and reparenting after the route’s cover has settled.

- **SHOULD-FIX — [cover.rs:302](kernel/src/kernel/cover.rs:302): the scroller boundary ignores computed overflow.** `overflow-x:hidden; overflow-y:visible` computes to vertical `auto` in the engine and Apple host, but `scrolls()` returns false. The walk can therefore cross that scroller and suppress padding for an inner `Inset`, violating Q1. Compute overflow using both axes and include `OverflowX` in the invalidation mask at line 340.

- **SHOULD-FIX — [id.rs:192](kernel/src/id.rs:192): published padding still accounts for the full suppressed cover.** With authored bottom padding 16 and a suppressed cover of 83, layout uses 16, but `resolved_padding()` reports 0 and `layout_padding()` reports 83. Retaining the route’s bottom cover exposes this in the new path. Share the applied-padding calculation between `apply()` and `BoxInset::of`, retaining the full cover separately for absolute offsets.

- **NIT — [cover.rs:358](kernel/src/kernel/cover.rs:358): unmirrored kernels pay for ancestor walks that cannot restyle anything.** Skip the walk when no layout engine exists; initial layout rebuild already derives styles from the final arena.

The sheet subtraction, including a nonzero top cover, and the focused keyboard-dismissal test look correct. Ordinary scroller reparenting updates both ancestors; covers set before layout survive reconstruction. Q2 holds for `Inset` changes.

Source review only; builds, tests, and timings were not run.
