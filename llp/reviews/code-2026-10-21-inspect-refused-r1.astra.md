# Code review: a refused inspect said in the Inspector, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `3bff3337df3ed31309c3205f2f399807ec4bd6a8965e1cf383e267959a60293e`), shared by both reviewers, each blind to the other's review; HEAD `9471bb7fa8`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 2 SHOULD-FIX, 1 NIT
- **Disposition:**
  Taken in the round 1 commit:
  - Astra 1, Grok 1 (BLOCKER: owning inspect was taken for inspecting; the server names a build's inspector on sending and keeps it after a refusal, so any listing erased a real refusal, and a fresh tool never showed one): ownership never clears a refusal. One clears on a selection from that build, on Inspect sent again, on re-registration (its status reset), and when another tool or none owns inspect there. Tested, including a status naming this tool's inspector, after which the line stays. Restoring the old clear-on-ownership fails the test.
  - Grok 3 (an Inspect that errored dropped the refusals it was retrying): the refusals before the send are restored on an error.
  - Astra 3, Grok 2 (a build re-registered under the same id kept an old refusal): a listing whose status is reset clears it. Tested.
  - Astra 2 (grid labels could outlive the layout in the poll's text): the label map is gone. The Inspector names a grid copy by joining the refusal with gridAt.tiles as it draws; otherwise the row's own name.
  - Astra 4 (the test relied on an earlier restart): it restarts the stub itself.
  The suite passes 67 of 67.

One blocker; the conditions are only partly met.

1. **BLOCKER — [app.ts:408](/tmp/rv-dt-insp1-astra/apps/design-tool/app.ts:408): ownership is mistaken for successful inspection.** The server sets `inspector` when sending the command and retains it after an `unsupported` report. Consequently, an ordinary `build-status` or `builds` event clears a genuine refusal without success or retry. Reopening the tool under the same client also seeds the recorded refusal in `buildOf`, then immediately deletes it in `owned()`, so it never appears. Clearing needs evidence newer than the refusal, not ownership alone.

2. **SHOULD-FIX — [app.ts:388](/tmp/rv-dt-insp1-astra/apps/design-tool/app.ts:388), [grid.ts:193](/tmp/rv-dt-insp1-astra/apps/design-tool/grid.ts:193): displayed names can outlive their grid labels.** The map clears each pass and contains only that pass’s shown tiles, but refusal text is materialized only by `waitChanges`. Changing modes, entering States, or leaving the grid does not refresh that text. Also, the map is empty or partially populated across awaited commands during layout. A poll can therefore capture an obsolete label or fallback ID until another poll completes. Publish labels atomically and make refusal presentation depend on the completed grid result.

3. **SHOULD-FIX — [app.ts:400](/tmp/rv-dt-insp1-astra/apps/design-tool/app.ts:400): same-ID registration can retain an obsolete refusal.** `buildOf` always preserves an existing build’s flag, even when registration resets the server’s `unsupported` record. If another client now owns that build, `owned()` does not clear it either. While this tool still inspects another build, the old warning remains visible. Distinguish a new registration from an ordinary status update.

4. **NIT — [app.test.contract:2319](/tmp/rv-dt-insp1-astra/apps/design-tool/app.test.contract:2319): the test relies on preceding cleanup.** The stub persists across tests; this test relies on the previous test’s restart. Reset it explicitly before opening the tool. Its clock sequencing and retry assertions otherwise look sound, but it does not cover status/list clearing, fresh-tool recovery, same-ID registration, or grid labels.

Selection clearing, explicit retry clearing, removal from the build list, the three-line/count limit, and the rendering guard while Inspect is off are implemented. The off-state empty text is unchanged. Preserving an existing cleared flag also prevents stale server `unsupported` data from resurrecting it during ordinary updates.

Read-only review; no builds or tests run.
astra exit 0
