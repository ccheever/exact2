# Code review: tamber/1.16 inherited rows' fromAt and inspect select, round 3 of 3 (Astra run twice: the first stopped at model capacity) (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort xhigh, read-only sandbox, a throwaway worktree.
- **Method:** round 3 of 3 (Astra run twice: the first stopped at model capacity); HEAD `c700c1d7a2`. The author (Claude) is not a reviewer.
- **Verdict:** nothing blocks; SHOULD-FIXes and NITs taken post-round in fc0ec2c006 and 618908d5a1.

Nothing blocks. Three issues remain:

1. **SHOULD-FIX — [host/web-js/inspect-dev.js:304](/tmp/rv-dt-inspwire/host/web-js/inspect-dev.js:304), [host/web/dev.js:909](/tmp/rv-dt-inspwire/host/web/dev.js:909):** The guard still accepts a changed selection when two repeated instances share a plan site. Select instance B while A’s move is in flight: the frame passes and moves B’s selection. Carry and validate the selected node/instance or a selection sequence.

2. **SHOULD-FIX — [apps/design-tool/app.ts:634](/tmp/rv-dt-inspwire/apps/design-tool/app.ts:634), [app.contract:948](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:948):** Page refusals arrive asynchronously after HTTP success, so `selectedUp.ok` remains true. They never populate `selection-up-error` or trigger `dropUp`; the next unrelated selection can still receive the “landed higher” note. The new refusal test covers only HTTP 409. Connect asynchronous refusals to the pending move.

3. **NIT — [apps/design-tool/app.contract:1841](/tmp/rv-dt-inspwire/apps/design-tool/app.contract:1841):** The note is hidden, not cleared, when selection changes. After landing on A, clicking B and then A resurrects it because A has the same key. Key it to the existing `inspected.visit` or clear it on departure.

Read-only review; no builds or tests run.
