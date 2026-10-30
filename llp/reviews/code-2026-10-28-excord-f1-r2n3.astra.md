# Code review: Excord F1, the fling's lead toward the top (x2/excord-f1), narrow re-check 3, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `48f9f4d96a`.
- **Method:** a narrow brief naming the earlier findings and their fixes; 2 rounds (app-only work in apps/excord, theme2's 08:32 confirmation of x2-perf4's approved fling plan), then narrow re-checks. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Note:** Answered in 4d87dbeab6: scrolled is main's 15-row rule again and leadOlder alone anchors from 100 rows (its away gate holds through a send's settling); a restore holds the lead with leadHeld instead of clearing topKey; the lead test now pins leadOlder (mutation-checked).

1. **P2 — `settling` can remain true and disable the 100-row rule.** The guard correctly suppresses momentum during a send’s return. However, `post` sets `settling` at [app.contract:810](apps/excord/app.contract:810), and only `scrollEnded` clears it, when `target == ""` and `endGap <= 0.5` ([618](apps/excord/app.contract:618)). The app explicitly documents that Windows sends no `scrollend`, and replacing a scrolling list can lose it ([106](apps/excord/app.contract:106)). The quiet fallback only resets hover/scrolling state ([1111](apps/excord/app.contract:1111)). Consequently, subsequent history scrolling can remain on the 15-row threshold. This exposes an existing lifecycle gap through the new guard.

2. **P2 — clearing `topKey` assumes the offset placement produces another report.** A concrete no-op case is re-selecting a channel while it remains exactly at its saved non-end position: `openChannel` reuses that key and offset ([484](apps/excord/app.contract:484)), then `landJump` clears the just-preserved `topKey` ([680](apps/excord/app.contract:680)). The documented scroll contract reports offset changes; an identical placement need not report ([contract-grammar.md:1541](docs/contract-grammar.md:1541)). Thus the combined fix still has a same-channel hole.

   The **already-captured values are safe**: `afterPage` copies its target and offset first ([427](apps/excord/app.contract:427)); clearing `topKey` does not overwrite `jumpFromKey` ([694](apps/excord/app.contract:694)). But while the key remains empty, subsequent `leaving` saves it ([557](apps/excord/app.contract:557)), another jump captures it ([642](apps/excord/app.contract:642)), and “above” fixture injections fall back near the history’s end ([inject.ts:71](apps/excord/fixture/inject.ts:71)). Position data and permission to resume the lead should be kept separate.

3. **Clean — the new `openChannel` assignment has no read-after-assign or ordering defect.** It compares the incoming ID with the existing `open`, preserves the existing key for equality, and precedes `open = id` ([460](apps/excord/app.contract:460)). The outgoing place is captured beforehand ([452](apps/excord/app.contract:452)). Finding 2 concerns the later placement, not this assignment.

Static review only; no edits, builds, or tests run.
