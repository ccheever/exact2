# Code review: Tamber 1.5, the inspect event (LLP 1122 T2b), round 1, 2026-10-16 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `e2fc330f6`.
- **Method:** one brief (sha256 `5c635b545c50487e7f34aa393683e5ff377f277b9ceeb1aa498ccf96d4d0daac`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (3 medium).
- **Disposition:** taken.
  - Server:
    - a page's exit from a command since replaced publishes nothing (no `selected: null`);
    - a `selected` report from a build no one inspects any more is refused (409) and not published, so a click on its way when Stop or a lapse ends inspect mode cannot bring a selection back;
    - each build record names its owner (`inspector`).
  - Tool:
    - whether it inspects a build comes from the records (hello's, `builds`, `build-status`) and the `inspect` events, never from a command's reply, which a newer event may follow;
    - `selected: null` no longer drops ownership (the event does);
    - a selection goes stale when its build is turned off with no one taking it in the same answer, whoever had it, and at once when this tool loses it.
  - Tests:
    - tamber-builds.test (records name their owner, off everywhere including an away build, a refresh says nothing);
    - tamber-inspect.test (a selection while not inspecting refused; a replaced command's exit publishes nothing);
    - the contract tests follow the events (the stub now publishes them).

---

- **Medium — Other clients’ off events leave selections editable.** [apps/design-tool/app.ts:1152](/tmp/rv-dt-ievent/apps/design-tool/app.ts:1152). An agent enables inspect, selects a node, then disables inspect or loses presence. The tool displays that agent’s selection, but ignores its off event for staleness because `e.client !== client`. Programmatic exits emit no `selected: null`, so the edit links remain available. Invalidate the displayed selection whenever its build loses inspect ownership, regardless of client; test another owner’s explicit off and lapse.

- **Medium — A delayed command reply can overwrite newer ownership events.** [apps/design-tool/app.ts:1151](/tmp/rv-dt-ievent/apps/design-tool/app.ts:1151), [1414](/tmp/rv-dt-ievent/apps/design-tool/app.ts:1414). The tool sends inspect-on; another client takes the build; the poll processes the ownership transfer; then the original command reply arrives and adds the build back to `inspectOn`. “Stop inspecting” and Outline remain enabled with no subsequent event to correct them. Merge replies and events using a shared server epoch/sequence, preserving recovery after missed polls. Test delayed replies across takeover and epoch changes.

- **Medium — A queued selection can undo the new off-event invalidation.** [apps/design-tool/app.ts:1146](/tmp/rv-dt-ievent/apps/design-tool/app.ts:1146), [scripts/tamber-builds.mjs:275](/tmp/rv-dt-ievent/scripts/tamber-builds.mjs:275). A page queues a selection report; the server processes inspect-off and publishes its event; the queued report then arrives. Selection validation checks plan/design/pin revision, which inspect-off does not change, so it publishes `selection` and the tool resets `stale` to empty. Carry the inspect command sequence on non-null selection reports and reject reports from ended inspect sessions. Test a report queued before off but delivered afterward.

The publisher covers all listed ownership mutations, suppresses refresh/no-op events, and emits losses before acquisitions. The 1.5 bump is consistent; remaining 1.4 references describe history. Missed polls and new epochs clear local inspect state, but unsequenced replies can repopulate it.

The added tests cover basic ownership transitions, not these ordering cases. They also omit event assertions for away-build off, replacement/eviction, refresh, and Outline state. Tests were read, not run; no files were changed.

**Verdict: request changes—ownership publication is complete, but client state and selection invalidation remain vulnerable to ordering errors.**