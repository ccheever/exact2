# Code review: LLP 1122 T5 step 2, the tool's set-row editor, round 1, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e3d912aff6`.
- **Method:** one brief (sha256 `38c94878355dfcb3ec69a43a4d4adc480b3e226a694d9004da21992c59f75c94`), shared by both reviewers, blind to the other review; commits f0f5e10a2a. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 1 medium; smaller points).
- **Disposition:** taken in `82d98133ff`: an edit of the selected copy's source (set-state, set-row, promote, detach) is refused at propose when the files the proposal reads are not the ones the selection was reported under, so a draft opened on old files can't be applied over a newer write in flight (T4's path too); the editor's heading names the attribute written, a shorthand's whole ("This template · 1 instance: border-radius"); the copy's key includes the build's facts (its pin, reload or plan), so a draft opened before a pin stays stale after reselecting; the undo words name a style's file; the test's style row is a paint row in its own file. Not taken: asserting the outgoing op's JSON (the contract DSL compares whole texts and has no escaped quotes; the op's fields are the join's address passed through, checked by the join tests) and a real compile through the stub (the gate runs the server).

**Changes requested: two issues block sign-off.**

1. **[P1] Style drafts are not bound to their original digests through proposal and Apply.**  
   [app.contract:877](/tmp/rv-dt-t5r1/apps/design-tool/app.contract:877) captures the opening context, but [app.ts:824](/tmp/rv-dt-t5r1/apps/design-tool/app.ts:824) proposes only the operations and adopts the response’s current digests.

   Concrete scenario: open `Panel.opacity` at digest F0; another client writes F1; type while that change’s poll response remains outstanding. The proposal reads F1, `stateStale` still sees F0 locally, and Apply sends F1 as its precondition. The old draft can overwrite the newer edit. I reproduced this client sequence in memory. Bind proposals and transactions to the selection’s captured digests, rejecting a mismatch before Apply. This exposes a weakness shared with T4’s path.

2. **[P2] The editor does not identify the shorthand it will replace.**  
   [inspector.ts:57](/tmp/rv-dt-t5r1/apps/design-tool/inspector.ts:57) supplies only the scope label, which becomes the heading at [app.contract:1547](/tmp/rv-dt-t5r1/apps/design-tool/app.contract:1547).

   With authored `border-radius=8`, the inspector displays `border-top-left-radius`; opening it shows “Edit this template · 1 instance”, but changing the value writes `border-radius`, affecting every corner. D6 explicitly requires labelling the control as the authored shorthand. Include `editable.row` in the editor’s label, and the state where present.

Two smaller points:

- **The inherited staleness guard can revive an old draft.** At [app.contract:699](/tmp/rv-dt-t5r1/apps/design-tool/app.contract:699), pinning the build makes the draft stale, but reselecting the same node clears it without reopening the row: [inspector.ts:89](/tmp/rv-dt-t5r1/apps/design-tool/inspector.ts:89) keys only build/node/site/instance. I reproduced this too. Preserve invalidation until the editor is reopened, consistent with the stated T4 rule.
- **The new test overstates its coverage.** [app.test.contract:1438](/tmp/rv-dt-t5r1/apps/design-tool/app.test.contract:1438) successfully edits `padding`, which the current join/server refuse as layout. Its style file also equals the selected node’s file. The [stub:207](/tmp/rv-dt-t5r1/apps/design-tool/stub/tamber-stub.ts:207) accepts arbitrary payloads and returns canned inverses. Use an allowed paint row in a distinct file; assert outgoing fields/types, structured diagnostics, and set-row draft invalidation. The current test checks refusal presentation and undo counts, not compilation, untouched bytes, or inverse restoration.

The remaining paths checked out: template operations preserve `{file, at, sha256, row, state?}`; style operations preserve `{style, row, file}` independently of the open design file. Numeric input becomes a JSON number; token names remain strings. T4’s value conversion is unchanged. Opening a row clears branch drafts and cancels opening or active scrubs; set-row offers no scrub. Apply retains the reviewed operation, and undo uses the returned inverse.

Verification used read-only Bun probes against the actual client functions with in-memory replies. I did not run the full Contract/browser suite. The checkout is unchanged.
