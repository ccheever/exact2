# Code review: LLP 1122 T3 steps 2-3, the component panel and the param rung, round 2, 2026-10-18 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `ebafe6aac`.
- **Method:** one brief (sha256 `c7862823cd23b256e5b21de922c7e504cfe7e4fe7c55b7cd31774ffd1dc0a7eb`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (4 medium).
- **Disposition:** all taken in `471ce36a4`: Apply writes the operations it reviewed (kept in the review), and `opsOf` reads the newest settled read of the app's design, never a missing one; that two-operation edit offers no scrub (`proposal.scrubbable`); a duration's select or field keeps its values as written, only its slider or stepper strips and writes ms; Only this mode after a panel edit starts its own branch draft; a branch editor edit (its value, a field, Only this mode) ends a scrub on another branch or the panel's; the panel shows on the app's design named by its file too. A contract test drives Only this mode on a param with no line (it fails with the scrub left on).

I would still request changes. Four correctness issues remain.

1. **[P2] A design refresh can drop the base operation from a reviewed conditional edit.** [app.ts:278](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:278) reads `snapshots.get(issued)`, but `loadDesign` increments `issued` before its snapshot exists. During that interval, `opsOf` silently returns only the conditional operation. [writeEdit:865](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:865) recomputes those operations instead of retaining the reviewed ones.

   For untouched `Card.fill`, review “Only this mode,” then let a file-change event start another design read. Apply during that read omits the required base line and is refused. My in-memory probe captured two operations in the review and one in Apply. Proposals made during the read have the same problem. Bind operations to the reviewed snapshot or retain them in the review.

2. **[P2] Duration normalization corrupts select values, including non-duration token names.** [components.ts:44](/tmp/rv-dt-t3r2/apps/design-tool/components.ts:44) strips trailing `ms` from every kind’s written value; [line 53](/tmp/rv-dt-t3r2/apps/design-tool/components.ts:53) then exposes that transformed value for selects.

   A duration default of `300ms`, with valid token options `duration.fast|duration.slow`, displays and prepends `300`. Returning to that option submits an invalid nonzero unitless duration. A colour select currently following `brand.ms` instead displays and offers `brand.`. Both reproduce through `paramRow`. Keep select values and token identifiers intact; normalize only numeric controls.

3. **[P2] “Only this mode” can adopt a panel value without establishing a branch draft.** [app.contract:864](/tmp/rv-dt-t3r2/apps/design-tool/app.contract:864) checks only operation and target before deciding whether to initialize `draftFor`.

   Open `Card.pad` with committed value `12px`, move its panel slider to `18px`, then press “Only this mode.” The panel correctly cleared `draftFor`, but this action leaves it empty while retaining `editValue = "18px"`. Consequently, `shownValue` displays `12px` while the proposal writes `18px`; `partsStale` also cannot protect this draft against subsequent branch changes. This transition needs explicit branch-draft ownership.

4. **[P2] Scrub cancellation is still missing when returning from the panel to the branch editor.** [cellText:839](/tmp/rv-dt-t3r2/apps/design-tool/app.contract:839), [partEdit:849](/tmp/rv-dt-t3r2/apps/design-tool/app.contract:849), and [onlyThisMode:862](/tmp/rv-dt-t3r2/apps/design-tool/app.contract:862) change the edit identity without cancelling an existing scrub.

   With `Card.pad`’s branch open, start an unconditional scrub through its panel slider, then press “Only this mode.” `editWhen` changes to `density=regular`, but the unconditional scrub remains open. The client reports “This scrub is of another edit,” disables Keep, and retains the old preview until Cancel. Round 1’s branch-to-panel direction is fixed; this reverse direction remains broken.

Smaller points:

- **Selecting the app design by filename hides its component panel.** [app.ts:809](/tmp/rv-dt-t3r2/apps/design-tool/app.ts:809) accepts only `file === ""`, although the picker also offers `app.tamber` explicitly. The probe returns two components for `""` and none for `"app.tamber"`. Compare against `designFile` as well.
- **A first conditional override still offers an enabled “Scrub live” button.** [app.contract:1418](/tmp/rv-dt-t3r2/apps/design-tool/app.contract:1418) does not account for its two operations. The data module correctly refuses before sending a scrub, but misleadingly says to review an already reviewed change.

The other round 1 corrections hold: icon objects no longer break loading; duration numeric controls use milliseconds; free-text kinds retain their values; different-param panel edits clear branch scope and drafts; token identifiers are distinguished from colour literals; and another design’s panel no longer writes the app design. The `STUB_APP` replacements are consistent, and the canned fixture’s source digests match both source files. The retained “Follows … / Pin” behaviour matches the recorded disposition. I found no new regression in ordinary dial or token controls.

For a settled design read, the client probe confirmed the base-before-`when` order, short-name operations, both operations retained through Apply/redo, and one inverse-stack entry for undo.

The **round 1 testing concern is only partly resolved**. The real-server Reset test checks byte restoration, and the later param-rung test covers editing a different panel param. However, the test at [app.test.contract:129](/tmp/rv-dt-t3r2/apps/design-tool/app.test.contract:129) opens a token despite its param-branch description. The fixture has no conditional param branch, and the [stub transaction handler:180](/tmp/rv-dt-t3r2/apps/design-tool/stub/tamber-stub.ts:180) accepts operations without applying them. Conditional ownership, cancellation, and the two-operation transaction remain unprotected.

Validation: both existing pure control/typed-field test bodies passed, alongside read-only Bun probes with mocked transport. I did not run suites that create files. The checkout is unchanged.
