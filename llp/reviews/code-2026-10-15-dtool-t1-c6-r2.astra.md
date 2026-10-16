# Code review: LLP 1122 T1 commit 6 (agents' edits and held proposals), round 2, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `f42acb24d` (commit `549504343` and its round-1 fixes).
- **Method:** one brief (sha256 `8c24e13f445550be51df30e5f79bee9552815834fe9c8b6aaa80c2595a5bef53`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** changes required (5 medium, 1 low).
- **Disposition:** taken but 2 and 6 in part: a highlight is per cell, by the latest client to change that cell (the cells that moved between the reads either side, else the token's row); the fade cursor is per file and run, and highlights fade oldest first, one each 3 s, so an unrelated edit never keeps one lit; an axis operation's item names its value, and every item its file (the design's when it names none). 2 in part: a hand edit no read could measure stops older clients being credited through it, but its own cells are not lit (naming its tokens needs the server to keep the files' texts). 6 in part: the stale accept now edits the proposed token and asserts the recomputed diff; the stub's recomputed proposal has its own diff and tokens; the first carrying poll is not counted.

---

Changes are still required. Reviewed `549504343` with the code at HEAD `f42acb24d`. Tests were inspected, not run; no files were changed.

1. **Medium — Attribution still identifies rows, not changed cells.** [app.ts:811](/tmp/rv-dt-6r2/apps/design-tool/app.ts:811), [app.contract:619](/tmp/rv-dt-6r2/apps/design-tool/app.contract:619).  
   File identity is now preserved, but `by` still maps each token to one client, and every column uses that colour. If Alice changes a token’s dark value and Bob then changes its light value, both cells say Bob changed them; untouched mode columns also highlight. This leaves Astra round-1 finding 4 partly unresolved. **Fix:** retain affected file/token/column identities and choose the latest author separately for each cell.

2. **Medium — A batched hand edit can inherit the preceding agent’s attribution.** [app.ts:564](/tmp/rv-dt-6r2/apps/design-tool/app.ts:564), [app.ts:810](/tmp/rv-dt-6r2/apps/design-tool/app.ts:810).  
   Suppose Alice writes a token red, then someone changes it to blue on disk before the next design read. The disk event has no token list, and `measure()` cannot derive one without the unread intermediate red snapshot. Walking backward therefore retains Alice’s token attribution and labels the displayed blue value as Alice’s. An unrelated hand-edited token instead receives no highlight. The batched-event fix does not fully resolve Grok finding 3. **Fix:** obtain token deltas for disk events independently of intermediate UI reads; do not carry older authors through an unmeasured disk change.

3. **Medium — Fading one file consumes another file’s unseen highlights.** [app.contract:507](/tmp/rv-dt-6r2/apps/design-tool/app.contract:507), [app.ts:806](/tmp/rv-dt-6r2/apps/design-tool/app.ts:806).  
   The fade cursor is epoch-specific but remains global across files. A transaction changes both `app.tamber` and `brand.tamber`; the person views `app.tamber` until its highlight fades, then opens `brand.tamber`. Its first read cannot highlight that transaction because its sequence is already behind `faded`. **Fix:** track displayed and expired changes per file, or per cell, so displaying one file does not consume another’s pending attribution.

4. **Medium — Unrelated edits still prevent highlights from expiring.** [app.contract:359](/tmp/rv-dt-6r2/apps/design-tool/app.contract:359).  
   Every new `lit.seq` restarts the shared three-second timer while retaining earlier unfaded tokens. After Alice changes A, Bob changing B every two seconds keeps A highlighted indefinitely. Astra finding 6 remains unresolved, contrary to the brief’s short highlight and the README’s three-second behavior. **Fix:** expire each highlight from its own appearance or latest relevant change; unrelated edits must not extend it.

5. **Medium — The new axis labels make distinct proposal items indistinguishable.** [app.ts:508](/tmp/rv-dt-6r2/apps/design-tool/app.ts:508).  
   The generic axis formatter prints `values` but omits singular `value`. Two operations adding `dim` and `navy` to `tone` both become `add axis value tone (b.tamber)`. The person cannot reliably choose which item to accept. This regresses the information previously available in JSON and leaves Grok finding 6 partly unresolved. **Fix:** format each axis operation explicitly, including its value and relevant placement or scope. Also resolve omitted `file` fields to the default design filename at line 511; Astra finding 8 is currently fixed only for explicitly named files.

6. **Low — The tests still do not establish first-poll attribution or fresh stale-diff review.** [design-tool.test.mjs:492](/tmp/rv-dt-6r2/scripts/design-tool.test.mjs:492), [design-tool.test.mjs:503](/tmp/rv-dt-6r2/scripts/design-tool.test.mjs:503), [tamber-stub.ts:79](/tmp/rv-dt-6r2/apps/design-tool/stub/tamber-stub.ts:79).  
   Attribution is inspected after two advances without counting carrying polls. The stale scenario changes `tone.warm` while proposing `accent.fill`, then checks replacement metadata without checking its diff. The stub still copies the old diff and affected-token list when filtering items. A regression showing the old review can pass. **Fix:** inspect rendered attribution after the first carrying poll; change the proposed token itself and assert the replacement’s new diff before acceptance; give stub replacements distinct, correct review data. Control response ordering when testing the accept/poll race.

The round-1 accept-operation capture, late-response session guard, restart fade reset, missed-poll baseline, and grid-width fixes are correct by inspection: Astra 1, 2, 5, 7, 9 and Grok 1, 2, 4 are addressed. Ordinary batched protocol writes and file-qualified attribution now work. The colour collision, `acceptedBy`, and `unset-token-when` fixes also hold. New byte comparisons meaningfully cover normal accept → undo → redo, and the stub checks rendered expiry and post-restart highlighting; the remaining partial fixes are identified above.

**Verdict: Changes required — 5 Medium findings and 1 Low finding.**
