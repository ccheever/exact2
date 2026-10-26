# Code review: the design tool's part outline (stage 12 D (a), LLP 1139 D5, on rt1's D4), round 1, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `c72cbad68d`.
- **Method:** one brief (sha256 `d9909cef704056b93d5dcf7d6d893be1748f7fcef10e6e85804683d4928ae8c9`), shared by both reviewers, each blind to the other's review; commit c72cbad68d. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 SHOULD-FIX, 2 NIT).
- **Disposition:** taken in the round's fix commit (named in round 2's records):
  - (1) The parts follow the app's describe while another file is open too (`loadAppAxes`, its own read counter, the open file's reads never moving it), and its answer refreshes the inspector (`appAxes.answer`); tested (a part gone while brand.tamber is open).
  - (2) The block says what is so: the rows are the package's recipe, read only; a token or param a row reads is set for this app (LLP 1139 D5's knobs, an app-design write, never a package file). Tested with a token-backed recipe row: its token offered, no row, state, param, tokenize or detach control; and the literal row's "Make it a token" absent. (review2's condition read as: no write of the recipe; the knob it reads is the library's API.)
  - (3) The fixture's groups are C1's (`paint`, `text`, `shape`, `space`).
  - (4) The Library line is a level-three heading.

No BLOCKER findings. Two SHOULD-FIX issues:

1. **SHOULD-FIX — Parts stop refreshing while another design file is open.** [app.ts:719](/tmp/rv-dt-s12a1/apps/design-tool/app.ts:719) updates `libraryParts` only through `loadDesign` of the app. While `brand.tamber` is open, [loadAppAxes](/tmp/rv-dt-s12a1/apps/design-tool/app.ts:1355) still describes the app, but consumes only its axes. If that describe removes `header`, changes the slot, or omits `parts` entirely, a fresh selection continues showing the old outline until the app design is reopened. Update parts from both app-describe paths under the same freshness guard, and make their completion refresh the inspector. Add a regression covering removal while another file is open.

2. **SHOULD-FIX — The blanket read-only statement and test miss token/param-backed recipes.** [app.contract:1630](/tmp/rv-dt-s12a1/apps/design-tool/app.contract:1630) says all these rows are read-only, while [the new fixture](/tmp/rv-dt-s12a1/apps/design-tool/app.test.contract:2016) supplies an already-refused literal row. For a package recipe reading `cards.fill`, the existing guard returns a package reason, but [`editable`](/tmp/rv-dt-s12a1/scripts/tamber-inspect.mjs:369) can still return a param or token rung. The inspector then displays **Edit**; an app override of that token is writable. This is inherited, intentional app-design editing—not a package-file write bypass—but it does not satisfy the stated blanket “no write control” condition. Cover actual joined token/param-backed recipe reports and align the new claim and acceptance condition with that behavior. The negative assertions also omit `selrow-tokenize-background-color`, the distinct **Make it a token** control.

3. **NIT — The describe fixture contains an impossible group.** [app.test.contract:2004](/tmp/rv-dt-s12a1/apps/design-tool/app.test.contract:2004) uses `groups: ["box"]`. C1 admits only `paint`, `text`, `shape`, `space`, and `control` ([anatomy.rs:21](/tmp/rv-dt-s12a1/contract/lower/src/anatomy.rs:21)). Use valid groups; the header’s background recipe needs `paint`. The enclosing `components[].parts` shape and `slot` boolean otherwise match D3’s plan.

4. **NIT — Give Library a semantic subsection heading.** [app.contract:1626](/tmp/rv-dt-s12a1/apps/design-tool/app.contract:1626) is ordinary footnote text, so heading navigation skips the new block. A level-three heading would fit under Inspector’s level two and match other subsections. The selected part is already stated in words, so its meaning does not depend solely on the triangle or color.

The other requested checks look sound on inspection:

- `part: null` retains the identity and marks no part. Non-object JSON values for `library` safely produce no outline. Lookup uses the declaring identity, preventing another component’s parts from leaking in.
- Held rows keep their own server-provided editability; they do not need `library` added to the view model for guarding. A library ancestor’s held rows should not turn the caller’s selected slot fill into a library selection.
- Stale selections retain informational text with the existing stale warning and action gating.
- `p.name` is a valid key: the compiler rejects duplicate part declarations and duplicate part nodes. Unusual text is interpolated as text, not interpreted as markup; production part names are restricted further.
- A successful app `loadDesign` replaces the map completely, including clearing omitted parts. The gap is the other describe path in finding 1.

The full real-server **describe → selection → outline** test should follow D3. D4’s existing compiled-map test already establishes the identity/part/fill shape. The guard coverage in finding 2 can be added now; it does not require D3.

Static review only; no files changed, builds, or test suites run.
