# Code review: LLP 1122, the design tool's reads follow the files, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `51702c2e8` (the change b9644d51c..51702c2e8).
- **Method:** one brief (sha256 `f4d70969420c8f71bc85fd0ce06f4208ce656ea9ebc8eb7a6beb110cb96c3110`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 1 low).
- **Disposition:** both taken, in `199e1beef`:
  - the proposal follows every poll answer (`changes.answer`, at least one each 25 s) except while a scrub is shown, in place of the builds key. The design reader getting ready sends no event, but the next answer, an idle one at the latest, asks again. The contract test still fails without it (checked with a constant in its place);
  - the chip takes the `live` it was asked with, not the module's last successful hello.
  - Also from Grok's smaller points: an overtaken read asks no further describe.

**One P2 still blocks.** The resolver pool and timeout fixes look correct, but proposal invalidation remains incomplete.

1. **[P2] Reader readiness can change the proposal class without changing any key** — [app.contract:565](/tmp/rv-dt-reread2/apps/design-tool/app.contract:565).

   The JS loop’s [inline check](/tmp/rv-dt-reread2/host/web-js/dev.mjs:617) returns no restriction until `toolFresh()` succeeds. Its background reader can finish after the page has registered; completion only [logs “design reader ready”](/tmp/rv-dt-reread2/host/web-js/dev.mjs:264). Files, registered builds and `hello.live` remain unchanged.

   Open a draft affecting an inline font-weight token while that reader is building: the proposal says Live. After the reader finishes, even subsequent build-status events leave that chip unchanged, although a fresh proposal returns Rebuilds. Scrub remains offered and is refused.

   I reproduced this using the actual `inlineWhy`, `proposeHere`, client and resource runtime. Restoring `since` to the proposal’s arguments corrects the chip on the subsequent status event. The proposal needs an invalidation signal covering classification readiness; expensive design reads can remain file-driven.

2. **[P3] A failed hello can leave the chip using stale live capability** — [app.ts:682](/tmp/rv-dt-reread2/apps/design-tool/app.ts:682), [app.ts:824](/tmp/rv-dt-reread2/apps/design-tool/app.ts:824).

   When live capability changes from true to false and the first hello fails, `loadHello` returns `live: false` but leaves `liveTable` true. The new argument re-runs `proposeEdit`, which ignores that argument and can produce Live from the old variable. A successful retry reporting false then changes no proposal key, leaving the chip wrong.

   This also reproduced in the resource runtime. Consume the passed live value when computing the chip. Scrub’s separate `hello.live` guard makes this smaller than finding 1.

The remaining requested checks passed:

- Indexed results preserve token order despite reversed completion order. Empty token lists work. A midway refusal returns cleanly, although workers finish requesting the remaining tokens.
- Overtaken reads stop issuing resolves after their outstanding requests finish. Their answers are dropped, including after a refresh with unchanged arguments. `loadAppAxes` does not increment `issued`.
- `state()` computes `buildsKey()` after processing events and any resynchronization. Ordinary `hello.live` changes correctly invalidate the dependent resource.
- Both new test scenarios detect their intended regressions: restoring `since` causes the describe refusal; omitting `buildsAt` leaves Live. The initial clock step makes default-zero `pass` correct. `classOf`/`INLINE` changes the stub response, and the ending restarts clear the shared refusal/build/live state. These tests do not cover either remaining finding.

Validation used in-memory probes with actual source, stub handlers and the JS resource runtime. Full Contract/browser tests were not run. No files changed.
