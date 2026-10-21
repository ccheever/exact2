# Code review: the design tool's half of stage 11 D, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `c3c3062230`.
- **Method:** one brief (sha256 `570a41fd1c866516ee6f8d1c2669d83a7f145be5829602d58ee8d3b0651dd775`), the plan inline, shared by both reviewers, blind to the other review; commit c3c3062230 (round 1's fixes) on the three before it. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 BLOCKER, 4 SHOULD-FIX).
- **Disposition:** post-round, taken in `6e3e8952ba`: an overridden base name is edited as the app's token only when explain names its branches (`whens`, asked of theme2 for 1.14 so the override's when lines can be read back); a base reply with the override's text alone shows it read only with why (Astra blocker 1, Grok 2); the re-scope reader offers none for a base explain (Astra 2); an unoverridden base value shows the first column the app ships (Astra 4, Grok 1); Set for this app is now set-token (theme2's 17:09Z ruling), which carries no kind, the server's to keep (Astra 3); the stub moves a package's row on set and back, and the test asserts an enabled editor holding the override, and the package's set and back (Astra 5, Grok 3); the stub's removal guards a missing name (Grok 4).

**One blocker remains.** Reviewed `c3c3062230` and the preceding three commits against both round-1 records and theme2’s brief. No files changed; no builds or test suites run.

1. **BLOCKER — Base overrides lose their mode branches.**  
   [app.ts:741](/tmp/rv-dt-s11dr2/apps/design-tool/app.ts:741) supplies `whens: []` for every base explanation, including overridden names. Suppose `space.md` defaults to `16px` and has `when density=compact: 8px`. Selecting its compact cell opens the default `16px`; changing it sends `set-token` with an empty `when`, changing the default while compact remains `8px`. The tool’s own “Only this mode” can create this situation. An overridden base needs its winning branch identified before offering ordinary cell edits.

2. **SHOULD-FIX — The re-scope reader still assumes the old explanation shape.**  
   [app.ts:610](/tmp/rv-dt-s11dr2/apps/design-tool/app.ts:610) passes base explanations unchanged to [rescope.ts:104](/tmp/rv-dt-s11dr2/apps/design-tool/rescope.ts:104). With exactly one component param aliasing an overridden `space.md`, applying an “Only this mode” edit reaches `lineOf`, which calls `.find` on the missing `whens`. Because re-scope preparation runs before the transaction, Apply fails before writing. An unconditional edit instead prepares an inverse with an undefined previous value. Handle base explanations here too, or decline re-scoping them.

3. **SHOULD-FIX — Package overrides drop the token’s required kind.**  
   [app.contract:1736](/tmp/rv-dt-s11dr2/apps/design-tool/app.contract:1736) passes only the name and value into adoption; [app.ts:177](/tmp/rv-dt-s11dr2/apps/design-tool/app.ts:177) emits `add-token` without `kind`. For `reading.shadow: box-shadow = 0px 1px 3px #00000033`, “Set for this app” therefore proposes an untyped shadow, which the checker explicitly rejects with `tamber-kind`. The corrected resolve-value seed is right, but adoption must also preserve the token kind.

4. **SHOULD-FIX — An unoverridden base explanation displays an arbitrary platform’s value.**  
   [app.ts:741](/tmp/rv-dt-s11dr2/apps/design-tool/app.ts:741) takes `Object.values(columns)[0]`, bypassing `targets`. For a Mac-only app receiving the brief’s column order, opening `space.md` through the inspector displays iOS’s `16px`, although the grid correctly shows Mac’s `10px`. Use the server-supplied column ordering or display the explicitly labelled platform values.

5. **SHOULD-FIX — Round-1 transition coverage remains incomplete.**  
   [tamber-stub.ts:313](/tmp/rv-dt-s11dr2/apps/design-tool/stub/tamber-stub.ts:313) updates only base `add-token`/`remove-token`; package adoption and subsequent `set-token` leave the fixture unchanged. The test at [app.test.contract:1874](/tmp/rv-dt-s11dr2/apps/design-tool/app.test.contract:1874) establishes “editable” solely by the absence of `branch-readonly`, which also passes if no editor opens. Assert an enabled editor and an actual edit, package adoption/removal transitions, and the ownership-change guards.

The other fixes look correct by inspection: supplied target columns are deduplicated without a target map; resolve values replace display-text seeds; read-only status invalidates branch drafts; the named controls are guarded; Set/Back select the app’s design. Package-override classification matches the recorded `file == hello.design` convention, pending A2 confirmation. Using the length editor for radius values is appropriate.

The not-taken decisions are reasonable: platform values can remain readable in the grid without another editor, and byte-exact undo belongs in the deferred real-server integration check.
