# Code review: LLP 1121 stage 9 G1, a look snaps on the web, round 4, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via Codex CLI, reasoning effort max (round 4: high), sandbox read-only, a throwaway worktree.
- **Scope:** the net change after round 3.
- **Method:** one brief (sha256 `ce0e0080d4d3d3171380dc27dd7ecf0a66337d1dc760429d25acb73d06521094`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** RIGHT WITH CHANGES (BLOCKING: the writeIn stub after navigation; the dedupe never consumed).
- **Disposition:** fixed in round 4's commit.

---

1. **Blocking — [host/web-js/rt.js:1153](/tmp/rv-g1-4/host/web-js/rt.js:1153):** Moving the `writeIn` stub fixes initial-tree ordering only. If a look loads input glue before the first GPU surface mounts—for example, after navigation—`gs()`’s `??=` at line 767 preserves the stub. Surface records then arrive as `("", null)`, losing their name and payload. **Fix:** let GPU initialization replace the input-only stub, or keep that fallback local to the link handler.

2. **Should-fix — [host/web/input-glue.js:486](/tmp/rv-g1-4/host/web/input-glue.js:486):** `pointerEdge` is never consumed or invalidated by mouse events. After pointer entry B→A and its compatibility event, Firefox’s mouse-only scroll crossings A→B→A incorrectly suppress the final entry into A. Its hover look eases instead of snapping. **Fix:** consume the matching compatibility edge and invalidate stale records when a different mouse edge arrives.

Both reproduced with the source logic in memory. Browser tests were not run under the read-only restriction.
