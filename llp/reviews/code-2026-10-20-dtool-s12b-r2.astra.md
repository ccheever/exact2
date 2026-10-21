# Code review: the design tool's "Set for this app" on a package component's params (stage 12 D (b)), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `13312a7783`.
- **Method:** one brief (sha256 `253632cdd92b60d0ff4fd279bf7560f13f1e0953641a34417d70c5d384442e3a`), shared by both reviewers, each blind to the other's review; commits 848102632c and 13312a7783 (round 1's fixes and part (a)'s post-round fix). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX, 1 NIT).
- **Disposition:** taken post-round, in the commit named in the sign-off request:
  - (1) The token a default names keeps the grid's own names too, a base name the app leaves unset included, beside the role resolution; such a base name is said as Exact's ("Edit Exact's base radius.md instead"); tested.
  - (2) The test checks each proposal is the param's own line (`proposal-tokens`, the op's name, for the edit and the reset); the stub's diff is one line of JSON that the test language can't match whole.

Nothing blocks. Two findings remain:

1. **SHOULD-FIX — The role-alias fix removes existing links for unset base-scale names.** [app.ts:739](/tmp/rv-dt-s12c/apps/design-tool/app.ts:739) now resolves defaults exclusively through `tokenOfIn(ir)`, which searches only `ir.tokens` ([rescope.ts:29](/tmp/rv-dt-s12c/apps/design-tool/rescope.ts:29)). Scenario: a package’s `Card.radius` defaults to `radius.md`, which the app has not overridden. That name exists in the grid’s `names` through [app.ts:682](/tmp/rv-dt-s12c/apps/design-tool/app.ts:682), but not in `ir.tokens`; the alternate link now disappears. HEAD~1 offered it. Preserve the known-name lookup alongside role resolution, with appropriate ownership wording.

2. **NIT — The proposal-test finding is only partly addressed.** [app.test.contract:2092](/tmp/rv-dt-s12c/apps/design-tool/app.test.contract:2092) exercises both controls and checks `editKind` and the full `editTarget`, but never checks the resulting proposal. Scenario: serialization sends `set-token` instead of `set-param`, or the proposal fails; these assertions still pass. Assert the proposal’s operation, target/value, and app-file diff for both edits. The stub already exposes the serialized operation in its diff ([tamber-stub.ts:337](/tmp/rv-dt-s12c/apps/design-tool/stub/tamber-stub.ts:337)).

The other recorded fixes check out:

- **Role spelling and ownership:** `-Exact-Label` resolves to `-exact-label`; the resolved token’s `package` determines the wording ([components.ts:99](/tmp/rv-dt-s12c/apps/design-tool/components.ts:99), [app.contract:2290](/tmp/rv-dt-s12c/apps/design-tool/app.contract:2290)). An app token such as `accent.fill` is no longer called the package’s.
- **Selected column:** both ordinary and “More” rows check membership before retaining `selColumn` ([app.contract:1835](/tmp/rv-dt-s12c/apps/design-tool/app.contract:1835), [app.contract:1841](/tmp/rv-dt-s12c/apps/design-tool/app.contract:1841)). Removing `3-dark` now makes the link use `firstColumn`.
- **Stub restart:** the first package post retains the original file map, and restart restores it whole ([tamber-stub.ts:454](/tmp/rv-dt-s12c/apps/design-tool/stub/tamber-stub.ts:454), [tamber-stub.ts:444](/tmp/rv-dt-s12c/apps/design-tool/stub/tamber-stub.ts:444)). Multiple posts and overwritten original keys are covered by that logic.
- **Parts freshness:** both app-read paths share `partsReads`; unrelated-file `loadDesign` calls neither increment it nor assign parts ([app.ts:645](/tmp/rv-dt-s12c/apps/design-tool/app.ts:645), [app.ts:725](/tmp/rv-dt-s12c/apps/design-tool/app.ts:725), [app.ts:1361](/tmp/rv-dt-s12c/apps/design-tool/app.ts:1361)). Returning from `brand.tamber` to the app invalidates the older app-axes read, preventing it from restoring a removed `header`. Staying on `brand.tamber` leaves the newest successful `loadAppAxes` able to publish parts; the unrelated-file read cannot starve it.

Static review only; no files changed, builds, or test suites run.
