# Code review: Excord warm address-following, round 4, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort xhigh, `-s read-only`, a throwaway worktree at `394e2889ef`.
- **Method:** one brief (sha256 `13bb847056b289c8ee5bc27e823f50003fb2aa1b20dfae8d5e5fe61e2c7d1ace`), blind to the other reviewer. The author (Claude, x2-perf4) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **MINOR — new regression:** [routing.contract:83](apps/excord/routing.contract:83). Narrow layout: open A → Back to the list → `/nope`. Back retains `open = A`; the new condition calls `stayOpen`, which pushes A again and rejoins it. An unmatched address unexpectedly reopens a channel the reader left. Check whether a channel route is showing, rather than only the retained `open` ID.

The round 3 claims check out in source:

- **MAJOR — resolved:** [routing.contract:84](apps/excord/routing.contract:84) restores A through `stayOpen` while B is pending. [routing.e2e.test.ts:237](apps/excord/fixture/routing.e2e.test.ts:237) asserts B is never open/kept and the final URL names general.
- **MAJOR — resolved:** [routing.contract:158](apps/excord/routing.contract:158) clears the picture after opening. [routing.e2e.test.ts:276](apps/excord/fixture/routing.e2e.test.ts:276) asserts help is open, `viewing` is empty and `covered` is false.
- **MINOR — resolved:** [routing.contract:138](apps/excord/routing.contract:138) returns an encoded duplicate to the canonical visit.
- **MINOR — resolved:** [app.contract:494](apps/excord/app.contract:494) records titles independently of the selected server; [app.contract:1220](apps/excord/app.contract:1220) reads them beneath another visit.
- **MINOR — resolved:** [routing.contract:37](apps/excord/routing.contract:37) defines `owned` independently of sign-out; both rollback guards use it at lines 47 and 61.
- **MINOR — resolved:** [server.ts:778](apps/excord/fixture/server.ts:778) drops the guild hold on explicit reset and preserves it for IDENTIFY.
- **NITs — resolved:** the beneath-visit comment is updated at [app.contract:1215](apps/excord/app.contract:1215), titles include `#`, and the unused `menu` helper is removed.

Production-router probes and both fixture-reset cases passed; the existing reset test passed. All 11 browser tests skipped because no web build exists here, so I did not independently verify the picture-test negative control. Worktree unchanged.

REQUEST CHANGES
