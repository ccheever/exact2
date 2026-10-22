# Code review: the design tool's "Set for this app" on a package component's params (stage 12 D (b)), round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `848102632c`.
- **Method:** one brief (sha256 `ad384fbe06f1336c3e28114fafa53acc97010b4a1587399e9be3ee6f8daca09d`) with two sections, A (part (a)'s round 2) and B (part (b)'s round 1), shared by both reviewers, each blind to the other's review; commits 848102632c. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited, both sections (this record's is its section).
- **Verdict:** nothing blocks (section B: 3 SHOULD-FIX, 2 NIT).
- **Disposition:** taken in the round's fix commit (named in round 2's records): (B1) the token a default names is found as the checker finds it, a role's spelling included (`tokenOfIn`); (B2) "the package token" only for a token a package defines (describe's `package` on its row), else "the token"; (B3) the panel's token opens in the selected column only when the design still has it, else the first; (B4) the stub's restart puts back FILES as before the first package post; (B5) the test edits a package param (`set-param`, its full name) and resets one (`unset-param`), and covers a role spelling and an app token against a package token.

**A. Part outline — round 2 of 2**

Nothing blocks. The recorded fixes are present, with one remaining freshness issue.

1. **SHOULD-FIX — An older app describe can overwrite newer parts after switching files.** [`app.ts:1358`](/tmp/rv-dt-s12b/apps/design-tool/app.ts:1358) returns for `file == ""` without invalidating `appPartsReads`, while [`app.ts:723`](/tmp/rv-dt-s12b/apps/design-tool/app.ts:723) updates the same map through `loadDesign` without participating in that counter. Scenario: an app describe starts while `brand.tamber` is open; the user returns to the app design, whose newer read removes `header`; the delayed earlier response then restores `header`. Dropping its resource answer does not undo the preceding module-level assignment. Both app-describe paths should share the parts freshness guard; unrelated-file reads should leave it alone.

The other dispositions check out:

- While another file remains open, `loadAppAxes` now replaces—and clears—parts, and [`appAxes.answer`](/tmp/rv-dt-s12b/apps/design-tool/app.contract:708) refreshes the inspector. The added regression covers removal in that situation.
- The corrected groups match C1, and Library is a level-three heading.
- The token-backed row assertions cover the offered token and absence of recipe-edit, state, promotion, and detach controls. The literal row now also checks “Make it a token.” Leaving held-row coverage unchanged is reasonable: held rows use the same conversion with their own server-provided editability.

Reading “no write control” as **no recipe write, while offering its token or param** is sound under the D5 distinction recorded in the brief and dispositions. [`editable()`](/tmp/rv-dt-s12b/scripts/tamber-inspect.mjs:366) deliberately preserves those rungs; their edits target the app design. The revised wording accurately distinguishes them. I could not independently compare the full LLP 1139 text because it is absent from this and the adjacent checkout.

**B. Package component params — round 1 of 2**

Nothing blocks. Three SHOULD-FIX findings and two smaller points:

1. **SHOULD-FIX — Role aliases lose the token option.** [`components.ts:72`](/tmp/rv-dt-s12b/apps/design-tool/components.ts:72) requires an exact name match. A package param default `"Label"` or `"-Exact-Label"` resolves to a declared `-exact-label` override, but neither spelling belongs to the token-name set, so no link appears. Resolve the default using the existing [`tokenOfIn`](/tmp/rv-dt-s12b/apps/design-tool/rescope.ts:29) semantics. Defaults naming another param correctly remain excluded: describe removes internal param tokens from `ir.tokens`.

2. **SHOULD-FIX — Component ownership is incorrectly presented as token ownership.** [`ParamControl`](/tmp/rv-dt-s12b/apps/design-tool/app.contract:2286) calls every matching default “the package token,” although the lookup retains no token provenance. For example, a package param defaulting to Exact’s `radius.md` gets that label because base names enter the same set. The new test also demonstrates the problem: its `accent.fill` is declared in the [app fixture’s theme](/tmp/rv-dt-s12b/apps/design-tool/stub/dials.tamber:35), yet [the assertion](/tmp/rv-dt-s12b/apps/design-tool/app.test.contract:2080) calls it the package’s. Preserve token ownership and use wording appropriate to the actual source.

3. **SHOULD-FIX — A stale selected column makes the panel’s token button do nothing.** [`app.contract:1833`](/tmp/rv-dt-s12b/apps/design-tool/app.contract:1833), also line 1839, reuses any nonempty `selColumn`. Scenario: select `3-dark`, then remove an axis so that column disappears. Clicking the param’s token option still passes `3-dark`; [`loadBranch`](/tmp/rv-dt-s12b/apps/design-tool/app.ts:747) returns an empty result, and no branch opens. Use the retained column only if it exists in the current design; otherwise use `firstColumn`. The initial, no-selection fallback itself is correct.

4. **NIT — Stub reset only removes the latest fixture’s file keys.** [`/__stub/package-component`](/tmp/rv-dt-s12b/apps/design-tool/stub/tamber-stub.ts:451) accumulates files but replaces `PACKAGED`; [restart](/tmp/rv-dt-s12b/apps/design-tool/stub/tamber-stub.ts:441) deletes only the latest payload’s keys. Posting `app.contract`, then replacing the fixture with one listing `src/card.contract`, leaves `app.contract` after restart. That changes subsequent package classification. Track all injected keys, or restore the previous file map. The current single-install test resets correctly, and its merged digests are consistent across read replies.

5. **NIT — The contract test checks wording and navigation, but not the package edit/reset proposals.** [`app.test.contract:2077`](/tmp/rv-dt-s12b/apps/design-tool/app.test.contract:2077) never edits a param or presses its reset. A wrongly wired operation could pass these assertions. Add checks for `set-param`, `unset-param`, and the app-file proposal diff. Existing generic param tests cover the shared write machinery, so this is a coverage gap rather than a demonstrated write defect.

The remaining requested checks look correct on inspection. [`components.ts:93`](/tmp/rv-dt-s12b/apps/design-tool/components.ts:93) preserves an app subdirectory component when its full home is listed, recognizes a vendored `@scope/pkg/...` identity as external, exempts `app.tamber#Name`, and avoids package claims when an older server lists no `.contract` sources. Param controls still default to `set-param`; “Back to the default” uses `unset-param`. The server’s diff—including file and hunk line numbers—is still [rendered unchanged](/tmp/rv-dt-s12b/apps/design-tool/app.contract:2015).

Static review only: no files changed, builds, or test suites run.
