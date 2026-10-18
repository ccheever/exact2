# Code review: stage 4 commits 3 and 4: the live table, round 2, 2026-10-15 (grok)

- **Family:** xAI. `grok-4.7` via the Grok CLI, reasoning effort xhigh, no subagents, a throwaway worktree, read-only by instruction.
- **Method:** one brief (sha256 `f9830548c200cfb10201b032f80a7b2772dfffe301ec9eea0897a7e0bdb63080`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocking (one docs note).
- **Disposition:** fixed in the second-round commit.

---

I'll review the last two commits as round 2: first the design and round-1 fixes, then a fresh pass over the live-table path. Read-only, no checkouts or edits.This is a custom read-only review of the last two commits, not a GitHub review. I'll start with the commit messages and the full diff, then check each round-1 fix against the live-table design.The two commits are the live-table feature and the first-round fixes. Next I’ll read the design and the value-only decision path, then check each claimed fix.Auto-compacting conversation (81% full)...
Conversation compacted.
I'll finish the round-2 review from the open edges: the tests HEAD claims, plan-reload state, and the timeout and signature checks.The role-override reset is only called from the JS emitter so far. I'll trace the runner reload path and the tests that claim to lock the round-1 bugs.Auto-compacting conversation (81% full)...
Conversation compacted.
The review is still open on a few edges. I'll check the review format and re-read those spots so the report cites the code, not the earlier notes.[P3] Update the propose section: it still says a live table is not applied — `docs/tamber-protocol.md:204`

`class` is described as what an edit would do to running development builds, and the next sentence says nothing applies a live table yet, so every edit still arrives as a dev-loop reload. The live-table section below (`docs/tamber-protocol.md:397`) is the behavior these commits ship: a value-only edit is a batch, and the running build does not reload. Anyone implementing `propose` from this section will treat `class: "live"` as unused.

Nothing blocking.

Round 1's fixes hold. The structure keeps every string the bytecode names, and a walk that cannot finish keeps the whole pool, so a shared code literal is not treated as value-only. Role overrides and `color(--…)` profile colours stay in the structure; a snapshot's role tokens are dropped before `set_tokens` (`runner/src/runner/live.rs:70`). Snapshots are applied in stream order on the web page, the JS target page, and Apple, and each of those fetches again after a generation lands. The TypeScript path compares the module executables, not the receipt (`host/web/dev.mjs:591`). A vanished path in the JS loop counts as a temporary file only beside an edited `.tamber`. A partial accept of a held proposal resubmits `p.guards` as `requires` (`scripts/tamber-server.mjs:527`), and the transaction rejects a digest that moved. `glue.js`, `host/web/src/host.rs`, and `Session.swift` each sit at 1,500 lines.

A change that needs a new plan is not taken as value-only. Contract text, surfaces, assets, axes, token gain or loss, a token that starts varying, and a synthetic token gained or lost all miss the value-only path. Ordinary colour edits, including a profile token's non-default slice, are applied in place: `edit_token` wins per tuple and the parse rebinds profile declarations. Bloom-bit collisions only reparse extra rows. Release builds refuse in `apply_live`. Writes wait with no client timeout (`scripts/tamber-server.mjs:446`).

Two residuals are already named for commit 5, not defects in this batch: a full wide-colour table still advances the design digest, and the JS loop's `--serve-as` producer does not relay a batch. The same-directory vanished-path test can also treat a real asset deleted in the same 5 ms burst as a design-file temporary (`host/web-js/dev.mjs:116`); a delete on its own, or in another directory, still rebuilds.
