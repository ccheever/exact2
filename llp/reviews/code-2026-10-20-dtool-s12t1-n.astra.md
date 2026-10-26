# Code review: the design tool's package param levels (stage 12 D follow-up T1), narrow verification round of round 2's post-round fixes, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `0c6fc168d4429e0a91dd065c7093e2b6c0ea7826e3acc5eed07cf4c78e496d35`), quoting round 2's two reviews verbatim, shared by both, each blind to the other's; HEAD `eec0282eb3`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER (A1 still open), 1 SHOULD-FIX (the fixture), the rest closed.
- **Disposition:** taken post-round, with review2's 17:16 early read (levels never block the design read), in the commit named in the sign-off request, and a second narrow round on it:
  - The BLOCKER (both reviewers): B2-levels' walk picks a `when` branch with no scheme side, so a tuple's level is the dark side's where a branch on an axis `under scheme=dark` decides it. The tool no longer leans on it. The level line names no column ("wins: …"). "The app's own value of this param still wins" shows only for a literal app line with no `when` branch, read from describe's `set`, which wins on both sides. The walk itself is asked of x2-perf4 and theme2 (to-x2-perf4.md and to-theme2.md 18:55).
  - The SHOULD-FIX on the fixture: Pill.tone's tuples are one package level (accent.fill has no branch). "Other modes differ" is now fill's: its compact branch names accent.soft, and the app overrides that token. A new Pill.mark carries the literal line that still wins.

**One BLOCKER and one SHOULD-FIX remain in HEAD `eec0282`.** The other round 2 findings are closed by inspection.

1. **BLOCKER — A1 remains open: winning every tuple does not establish light-side provenance.**  
   The new `self` check at [app.ts:152](/tmp/rv-dt-s12tn/apps/design-tool/app.ts:152) still permits a false assurance. With `dark-style = black | dim under scheme=dark`, extend the original scenario to:

   ```text
   Pill.fill = accent.soft
     when dark-style=black: #000000
     when dark-style=dim: #111111
   ```

   B2’s [walker](/tmp/rv-dt-s12tn/contract/cli/src/tamber_levels.rs:134) reports this param’s `app-param` for **every tuple**, making `self` true. However, the [fold](/tmp/rv-dt-s12tn/tamber/src/check.rs:1232) preserves the base value’s light side. Removing the app’s red `accent.soft` override still changes Light to the package’s blue, despite [the note](/tmp/rv-dt-s12tn/apps/design-tool/app.contract:2315) saying the param’s own value “still wins over it.”

   The original one-branch scenario now gets conservative reset wording, but its misleading “wins in Light” label also remains. Use conservative wording unless provenance distinguishes schemes. The new tests do not exercise this counterexample.

2. **SHOULD-FIX — A3 is only partially closed: the replacement fixture still contains an impossible winner.**  
   [app.test.contract:2119](/tmp/rv-dt-s12tn/apps/design-tool/app.test.contract:2119) now declares `Pill.gap` and makes `Pill.inset` reference it, closing that inconsistency. But `Pill.tone` now alternates to `role / -exact-label`: the stub’s [`accent.fill` has no branches](/tmp/rv-dt-s12tn/apps/design-tool/stub/dials.json:3006), and [`-exact-label` is overridden to `text.primary`](/tmp/rv-dt-s12tn/apps/design-tool/stub/dials.tamber:55). B2 cannot produce that role winner from these definitions.

   The refusal portion **is closed**: [the stub checks `refuse` before its success fallback](/tmp/rv-dt-s12tn/apps/design-tool/stub/tamber-stub.ts:320), and [the new test asserts the resulting design error](/tmp/rv-dt-s12tn/apps/design-tool/app.test.contract:2160). Make the remaining fixture definitions and replies agree.

3. **SHOULD-FIX — A2/B1 closed: filename selection restores param resolves.**  
   [app.ts:121](/tmp/rv-dt-s12tn/apps/design-tool/app.ts:121) accepts both `!file` and `file === designFile`, matching the component-panel condition. Both selections retain the same bounded, digest-checked resolve path.

4. **NIT — A4 closed: missing provenance is explicit.**  
   [app.ts:152](/tmp/rv-dt-s12tn/apps/design-tool/app.ts:152) records an unavailable level for empty tuples or a missing first level; [components.ts:136](/tmp/rv-dt-s12tn/apps/design-tool/components.ts:136) renders the explanation. [The new assertion](/tmp/rv-dt-s12tn/apps/design-tool/app.test.contract:2132) covers the missing-level reply.

5. **NIT — B2 closed: the package note no longer implies branches exist.**  
   [app.contract:2315](/tmp/rv-dt-s12tn/apps/design-tool/app.contract:2315) now says “with any when branches it has.”

No additional regressions found in the inspected changes. No files changed; no builds or test suites run.
