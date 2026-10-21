# Code review: LLP 1122 T7b, the narrow verification round of a51522a217 and c840f90d17 (review2's 00:39Z ask), 2026-10-20 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `c840f90d17`.
- **Method:** one brief (sha256 `6c0eab4d83eccf2fada56927910043062464a90cf9c7b8b45d2a6ec62b411892`): round 2's findings verbatim, then "is each closed by those commits, and did they break anything?". Shared by both reviewers, each blind to the other's review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** round 2's findings mostly closed; A1 and A2 partly (as recorded, below); 3 new SHOULD-FIX.
- **Disposition:** small fixes, in the commit named in the sign-off note (review2 reads them, no further round):
  - A copy reloaded at a place, its build unable to navigate, is reloaded again at each new place (new 1).
  - States compares the selection object, not its `e`: a hold moving the selection's report is not a new choice (new 2).
  - The larger text size takes 4 to 64 px as the field does; the third size shows only where it fits (new 3).
  - A copy sent somewhere has gone once it says any new place, or five passes on, and isn't sent there again, so a router's rewrite or a Back doesn't loop (B4).
  - A later navigate refusal reloads the copy only while navigation is still mirrored (A9).
- **Not taken:**
  - A server-side conditional release (A1, B5): the server's `pin` has no revision condition. The grid releases only a pin whose revision is its own or not yet listed, and leaves another client's later one.
  - The first command of a run unbound until it succeeds (A2): until then the grid holds no duty a new server's token could act on, and each run's duties are cleared when its epoch changes.
  - A stale same-origin word (A8, B8), as recorded in round 2.
  - The stub's holds and the lifecycle tests (A11, B7), as recorded.

**Blockers remain: A1 and the remaining restart gap in A2.** Static review only; no changes, builds, or tests. No additional issue found in HEAD’s app-note split.

1. **PARTLY — A1:** [grid.ts:121](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:121), [tamber-builds.mjs:535](/tmp/rv-dt-t7b3/scripts/tamber-builds.mjs:535): pin acknowledgements now retain revisions, but releases remain unconditional and holds still use value equality; the explicitly untaken ownership protection remains a blocker.
2. **PARTLY — A2:** [grid.ts:111](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:111), [wire.ts:128](/tmp/rv-dt-t7b3/apps/design-tool/wire.ts:128): existing duties are bound, but the first command remains unbound until success, so its retry can still acquire a restarted server’s token.
3. **CLOSED — A3:** [grid.ts:200](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:200): States waits for the selected location before recording or sending its hold attempt.
4. **CLOSED — A4:** [grid.ts:169](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:169), [grid.ts:126](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:126): mismatched plans trigger cleanup, and a remapped site no longer prevents release.
5. **CLOSED — A5:** [grid.ts:197](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:197), [grid.ts:204](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:204): reselection changes the request key; unsuccessful sends remove it, permitting retry.
6. **CLOSED — A6:** [app.ts:1081](/tmp/rv-dt-t7b3/apps/design-tool/app.ts:1081), [grid.ts:210](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:210): host navigation refusals now retire pending navigation and schedule fallback.
7. **CLOSED — A7:** [grid.ts:135](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:135): changing view or toggling Mirror clears obsolete pending navigation.
8. **NOT TAKEN — A8:** [grid.ts:55](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:55): registration still stamps the desired `src` without proving document generation; explicitly left unchanged in the disposition.
9. **PARTLY — A9:** [grid.ts:215](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:215): superseded sends stop, but a delayed refusal still sets `reloaded` after Mirror is disabled; the next pass can reload to that obsolete destination.
10. **CLOSED — A10:** [grid.ts:133](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:133): an empty epoch from failed hello no longer clears duties.
11. **PARTLY — A11:** [app.test.contract:1986](/tmp/rv-dt-t7b3/apps/design-tool/app.test.contract:1986), [tamber-stub.ts:243](/tmp/rv-dt-t7b3/apps/design-tool/stub/tamber-stub.ts:243): event-based refusal coverage was added; ownership, document replacement, exact hold payloads, and delayed lifecycle coverage remain missing.
12. **CLOSED — A12:** [vocab.rs:1187](/tmp/rv-dt-t7b3/contract/cli/tests/it/vocab.rs:1187): the comment now limits its claim to tested tags and acknowledges native buttons.
13. **CLOSED — B1:** [grid.ts:126](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:126), [grid.ts:169](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:169): a plan mismatch releases the hold even when the server changed its site index.
14. **CLOSED — B2:** [grid.ts:197](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:197), [grid.ts:204](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:204): same-view reselection rearms the request, and failed sends no longer permanently consume it.
15. **CLOSED — B3:** [grid.ts:179](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:179): a copy’s first location initializes observation without becoming the leader.
16. **PARTLY — B4:** [grid.ts:178](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:178), [grid.ts:212](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:212): rewritten arrivals clear pending navigation but immediately resend the original target; Back to the previously observed location never clears it.
17. **PARTLY — B5:** [grid.ts:117](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:117), [tamber-builds.mjs:535](/tmp/rv-dt-t7b3/scripts/tamber-builds.mjs:535): fresher snapshots and revisions narrow the race, but another client can still replace the pin before unconditional deletion.
18. **CLOSED — B6:** [grid.ts:133](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:133): failed hello preserves the original run and its cleanup duties.
19. **NOT CLOSED — B7:** [app.test.contract:2039](/tmp/rv-dt-t7b3/apps/design-tool/app.test.contract:2039), [tamber-stub.ts:246](/tmp/rv-dt-t7b3/apps/design-tool/stub/tamber-stub.ts:246): the added navigation assertion does not exercise hold remapping, reselection, or competing owners; the deficient hold stub remains unchanged.
20. **NOT TAKEN — B8:** [grid.ts:55](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:55): the same-origin stale-document registration window remains, as explicitly recorded.
21. **CLOSED — B9:** [vocab.rs:1189](/tmp/rv-dt-t7b3/contract/cli/tests/it/vocab.rs:1189): the comment acknowledges the existing native-button exception.
22. **NEW SHOULD-FIX:** [grid.ts:212](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:212): `!reloaded.has(t.name)` prevents every subsequent navigation after the first fallback; a copy reloaded at `/a` cannot follow the next leader move to `/b`.
23. **NEW SHOULD-FIX:** [grid.ts:142](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:142), [app.ts:345](/tmp/rv-dt-t7b3/apps/design-tool/app.ts:345): releasing a hold can clone the current selection’s `e`; this increments `chosen` and reapplies the hold without user reselection.
24. **NEW SHOULD-FIX:** [grid.ts:65](/tmp/rv-dt-t7b3/apps/design-tool/grid.ts:65), [app.contract:1560](/tmp/rv-dt-t7b3/apps/design-tool/app.contract:1560): the input still accepts 59–64px, but the new 58px cutoff silently removes all enlarged tiles for those previously valid values.
