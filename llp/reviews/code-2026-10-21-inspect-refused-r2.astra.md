# Code review: a refused inspect said in the Inspector, round 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `99d252a5ff6f56b4a7b55744916f6c60ba51bab256295ff3f802db2c89e701ce`), shared by both reviewers, each blind to the other's review; HEAD `6ccd88dfd6`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; 4 SHOULD-FIX, 1 NIT
- **Disposition:**
  Neither reviewer found anything blocking. Their SHOULD-FIXes are taken post-round, in their own commit:
  - Astra 1, Grok 1 (ownership lost through an `inspect` event, a page leaving inspect itself or a takeover, left the line; a refusal from a build another tool inspects was taken): the inspect event that takes a build from this tool clears its refusal, and an `unsupported` is taken only for a build this tool inspects. Both tested.
  - Astra 4, Grok 2 (a failed re-send could restore a refusal a newer event had cleared): refusals are counted per build (`refusalSeq`), and a failed send restores only those nothing has moved since.
  - Astra 3, Grok 3 (re-registration from a never-reported status was missed): a listing with its status reset and no recorded refusal of inspect clears it. Tested with a build that refused before its first status.
  - Astra 5 (one-copy mode could use a hidden grid's label): the join is guarded by `canvasView != "one"`.
  - Astra 2, Grok 4 (fresh-tool seeding can show a refusal the server still records after a later success, until the build's status moves): kept, and said here as a limit. Without it, a reloaded tool shows no refusal at all, the original invisibility. The server keeps the record only while the build's status stays at that revision.
  The contract suite passes 67 of 67.

Nothing blocks. Round 1’s ownership BLOCKER is closed, but some clearing and recovery cases remain incomplete.

1. **SHOULD-FIX — [app.ts:1134](/tmp/rv-dt-insp2-astra/apps/design-tool/app.ts:1134): ownership loss does not clear refusals through actual events.** Takeover and page-local exit emit `inspect` events, which update `inspectOn` without clearing `inspectRefused`. With another build still being inspected, the old warning remains until a listing arrives. A page leaving Inspect does relinquish ownership; clearing is appropriate. The test at [app.test.contract:2361](/tmp/rv-dt-insp2-astra/apps/design-tool/app.test.contract:2361) substitutes a `build-status`, missing this path. Additionally, `unsupported` at line 1118 accepts another owner’s subsequent refusal without checking ownership.

2. **SHOULD-FIX — [app.ts:400](/tmp/rv-dt-insp2-astra/apps/design-tool/app.ts:400): server-record recovery remains inconsistent.** For a known build, resync ignores a recorded refusal if its event was missed—part of Grok’s round-1 finding remains open. Conversely, fresh-tool seeding can resurrect a cleared refusal: the server retains `unsupported` after a successful selection or retry. When those events have left the retained log, hello seeds obsolete information. Recovery needs evidence of which inspect attempt the record describes.

3. **SHOULD-FIX — [app.ts:400](/tmp/rv-dt-insp2-astra/apps/design-tool/app.ts:400): re-registration clearing requires an earlier status.** A build may refuse Inspect before reporting its first status. Re-registering that ID resets the server record, but both old and new statuses are null, so `was.status != null` prevents clearing. The test covers only a non-null-to-null transition.

4. **SHOULD-FIX — [app.ts:1436](/tmp/rv-dt-insp2-astra/apps/design-tool/app.ts:1436): error restoration can undo newer clearing evidence.** While the request awaits its response, a selection, re-registration, or ownership-changing listing can legitimately clear the refusal. If the request then fails, the null check restores the obsolete warning. Restore only when no newer event has superseded the saved refusal.

5. **NIT — [app.contract:1689](/tmp/rv-dt-insp2-astra/apps/design-tool/app.contract:1689): one-copy mode can temporarily use a hidden grid’s label.** Switching modes hides the grid immediately, while `gridAt` retains its previous result during awaited pin/hold releases. Guard the join with `canvasView != "one"`. States and tiles without builds otherwise join correctly.

The test now initializes its own stub and meaningfully covers refusal persistence, selection clearing, the three-row limit, and successful retry. Its ownership scenario is incomplete as described above; error restoration and fresh-tool recovery remain untested.

Read-only review; no builds or tests run.
astra exit 0
