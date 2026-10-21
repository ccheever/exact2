# Code review: stage 4 commit 1: reclaimable wide colour storage, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree, read-only by instruction.
- **Method:** one brief (sha256 `d606b85060061a33eb0aaa7089326302bf20ac19296dc32483ac0e44e73ac4d3`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer. Round 1 (x2-perf3's, before the 2026-10-15 restart) left no transcript.
- **Transcription:** the final message, unedited.
- **Verdict:** changes requested (the document tree's keep marked an empty tree; the reorder keep outlived its ops).
- **Disposition:** fixed in the version reviewed in round 3.

---

I'll review the HEAD commit read-only, starting with the review instructions and the commit itself.This is a read-only review of the HEAD commit. I'll inspect the commit and the wide-colour reclaim path against the six hunt items.The commit is the wide-colour mark-and-sweep. Next I’ll read the table, the holders, and every place a `ColorValue` can outlive a call.Auto-compacting conversation (80% full)...
Conversation compacted.
The review is still open. I’ll confirm the DocTree keep and the other hunt items against the current HEAD before reporting severity.[P1] The kernel-free document tree's keep runs while the node list is still empty — `runner/src/instance/document.rs:167`

`Tree::document` installs `doc.kept` before `children()`. `Keep::of` runs the visitor immediately (`kernel/src/style/wide/reclaim.rs:334`). `nodes` is still the empty vec from `DocTree::default()`, the mark stays empty, and an empty mark registers no holder (`reclaim.rs:337`). `children` then folds each style (`document.rs:361`) and `push` stores it (`document.rs:322`). Every wide colour parsed there is interned and then lives in the returned tree with no mark.

That tree is the kernel-free document (`runner/src/runner/document.rs:11`). A detached kernel still registers a `Holder` (`kernel/src/kernel.rs:308`, `kernel/src/kernel/document.rs:11`), so the table does sweep. `host/render` reads those styles back through `color_css` → `wide()` (`host/web/src/css.rs:660`, `kernel/src/gradient.rs:689`). Once the table is down to its last quarter of slots, a sweep frees any unmarked id older than `GRACE` (1,024) interns. One `document()` that interns more than 1,024 distinct wide colours does that to its own earliest colours before `project_tree` reads them, and so does any `DocTree` kept across that many later interns. `wide()` returns `None`, and a debug build asserts (`kernel/src/style/wide.rs:63`), so the colour is missing.

Moving the keep to after `children()` covers the tree once `document()` has returned. Colours interned while the fold is still running stay grace-only until that keep, so a fold past 1,024 distinct colours still needs the mark published as nodes are pushed.

[P3] A reorder keep stays registered after its ops are taken — `runner/src/runner.rs:1390`

`drop_reorder_now` marks `reorder_ops` (`runner/src/runner/reorder.rs:224`). The error path then `mem::take`s that vec and applies it (`reorder.rs:233`). `apply` and `update_into_views_inner` do the same take (`runner.rs:1390`, `runner/src/runner/into_view.rs:175`) and leave `reorder_kept` in place. The queue no longer holds those ids; the keep marks their slots until the next reorder replaces it. After the commit, the kernel's mark covers them, so the leftover keep only pins slots the kernel has since dropped.

The DocTree keep is the blocking defect. The reorder pin is a slot leak.

No other missed holder turned up. Linux content-region workers receive `[u8; 4]` palettes captured on the UI thread. Apple, web, web-js, and the terminal convert or resolve at the boundary. Theme tables store colour text. Motion converts a wide endpoint to an Oklab value when the transition is adopted (`kernel/src/motion.rs:268`). Role overrides are kept before the table is replaced (`kernel/src/style/roles.rs:293`). An arena clone visits its styles at clone time (`kernel/src/arena.rs:174`). The Linux lookahead copies default colour rows over the scratch ops (`host/linux/src/presenter/ahead.rs:239`; `StyleProps::clear` writes defaults in `kernel/build.rs:1064`).

`Settle` follows `Colors` for gradients, shape paint, items (markers included), and scenes. Gradient interpolation is fixed before the stops become 8-bit (`kernel/src/style/wide/marks.rs:255`). Linux admits only in-sRGB wide colours, so the remaining difference on a ramp is that quantization; the settled-gradient test bounds one ramp to 2/255. A scheme flip re-captures a picture whose recorded appearance no longer matches. SVG paint resolves each stop, and the retained filter chain is already rgba.

Generation checks, the free list, `by_text`, `live`, `stirred`, refusal counting, and the table-lock-then-`stale()` order in `reclaim.rs` match the design. An ordinary runner commit marks once at the end of the batch. The document fold above is the only real host path that interns an unbounded run of distinct wide colours before any mark. No path hands an id to another thread.

`kernel/tests/wide.rs` covers reclaim, a half-grace in-flight id, an idle kernel, a keep, and refusal counts. Its motion check shows the painted midpoint after the red id is reusable, which holds because the transition stores the Oklab value. `kernel/tests/wide_stale.rs` checks one unmarked id. The Linux test checks retained frame bytes after a sweep and B's text ink across a scheme flip; it does not compare gradient samples with an unsettled ramp. None of them build a `DocTree` and sweep.

Commit marks still publish the cached style-slot union. A sweep runs when free slots are at or under a quarter and a mark has changed or another 1,024 colours have been interned. Nothing on that path looks blocking.
