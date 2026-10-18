# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 3, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `5c7138e94` (before dt/t2's rebase onto main; it is `2d59d8578` after, the shas named in the briefs map as /tmp/dt-t3/sha-map-1.txt says).
- **Method:** one brief (sha256 `bb57ca48414d0065e045f26fdcb4fc834e22ec017d783fd3f5d5f01d390f91ae`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 high, 2 medium).
- **Disposition:** taken in `f044d508a`: a param's first when line is proposed only from the params of the proposal's own revision (else 'the design changed: reading it again'); a default that reads another param (named in full) is no line Only this mode can write, and the proposal says so; a param named in full is an expression's input in the branch editor (Follows / Pin); a length or number slider or stepper with no amount to start from (a rem, a token) is a field; the stub's duration param rebuilds. Kept: the two-operation edit's disabled Scrub has no sentence of its own (the button is disabled, nothing is sent).

I would still block this round. Three correctness issues remain.

1. **[P1] “Only this mode” can overwrite another client’s unconditional value during a refresh.** [app.ts:280](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:280), [app.ts:855](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:855).

   Load untouched `Card.fill`, then let another client set its base to `#00ff00`. While the refreshed design is pending, `appParams` still says there is no base line. The proposal therefore prepends `Card.fill = accent.fill`, but receives the **new** server digests. Apply before the refresh finishes replaces the other client’s base, changing modes outside the requested branch.

   I captured both the proposal and Apply payload with this combination of old metadata and new digests. Retaining reviewed operations fixes round 2’s unchanged-refresh failure; it does not make the metadata used to construct those operations consistent with the proposal’s revision.

2. **[P2] References to another param do not survive the new editing path correctly.** [app.ts:281](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:281), [app.ts:204](/tmp/rv-dt-t3r3/apps/design-tool/app.ts:204).

   A valid default such as `color-mix(in oklab, param.tone 88%, black)` is presented by `describe` as `color-mix(in oklab, app.contract#Card.tone 88%, black)`. For its first conditional override, `opsOf` copies that display string directly into the new base operation. The server translates operation **names**, but does not translate these value references; its CSS value parser cannot read that full param name. Thus this valid param’s first “Only this mode” proposal is refused. Selecting a sibling-param option exposed by `describe` has the same problem.

   Separately, `inputsOf` rejects words containing `#` and splits paths at `/`. My `loadBranch` probe returned `expression: false` and no inputs for that dependent default. The branch editor consequently omits “Follows … / Pin” and permits replacing the expression without the explicit pin D6 requires.

3. **[P2] A relative-length stepper displays and edits a fabricated zero.** [components.ts:58](/tmp/rv-dt-t3r3/apps/design-tool/components.ts:58), [app.contract:1650](/tmp/rv-dt-t3r3/apps/design-tool/app.contract:1650).

   A valid, unranged length param with `default="1rem"` and `control="stepper"` produces `value: "1rem"`, but `number: 0` and `unit: "px"`. The numeric field displays zero; incrementing once proposes `1px`. Both the written and folded values can legitimately remain relative—only **ranged** lengths must be px. An unavailable pixel amount must not silently become zero.

The earlier testing concern remains partly unresolved:

- [app.test.contract:1144](/tmp/rv-dt-t3r3/apps/design-tool/app.test.contract:1144) checks the mode and disabled scrub button, but never applies, undoes, inspects both operations, or switches from an active scrub. The fixture still has no conditional param branch, and the stub still accepts transactions without applying them.
- [tamber-stub.ts:37](/tmp/rv-dt-t3r3/apps/design-tool/stub/tamber-stub.ts:37) omits the new duration param `MapCard.pan` from `REBUILDS`, so that fixture incorrectly classifies its edits as live.

The other earlier corrections hold: icon loading; duration and free-text controls; duration-select preservation; exact token-option matching; app-design-only panels, including explicit filename selection and native markers; panel draft/scope clearing; and the cancellation guards in both directions. The param remains the primary inspector rung. Apply/redo retain the reviewed operations, one undo covers the transaction, and two-operation proposals disable scrubbing. The ordinary colour-alias “Follows … / Pin” behaviour matches the recorded disposition. `STUB_APP` replacements and fixture source digests are consistent.

Validation used the two existing pure test bodies, in-memory control/resource probes, and source tracing. I did not run server/browser suites that create files. The checkout is unchanged.
