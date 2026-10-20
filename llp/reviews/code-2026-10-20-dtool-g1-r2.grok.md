# Code review: the variant gallery's G1, variant props in the component panel, round 2 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `bcdfb0ca8541a7c7eb61fdf690d2dfbc13fdc41701882599440767a1362d5c1c`), shared by both reviewers, each blind to the other's review; HEAD `debe99a923`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks; no findings.
- **Disposition:**
  - none: round 2 had no findings from either reviewer.

I'll review the design-tool variant-gallery commit read-only: confirm the two round-1 nits are closed, then check the whole change against the describe contract and the panel's other component walks.The review stays on `HEAD~1..HEAD` only. I'll confirm the two earlier nits are gone, then re-read the variant listing against describe's shape and the other component walks.Round-1 edits are in HEAD. Next I’ll read the panel, describe’s `variants` shape, and every other walk that assumes a listed component has params.Round 1 is closed. Nothing blocks G1.

**Round 1**

1. Closed. The heading at `apps/design-tool/app.contract:1866` is `Variants: the options each prop takes (the tool pins none on an instance)`. It names no LLP and does not say pins come later.
2. Closed. The comment at `apps/design-tool/app.test.contract:2208` only describes the next assertion: Badge has no param group (`param-group-Badge-`). It no longer claims the test proves there is no control and no selected value.
3. Closed. `apps/design-tool/app.contract:526` now says the panel list is each component with params or variant props.

**Whole change**

No findings.

`components()` emits `variants: [{prop, options}]` with string options, and a library component with no params is listed with `params: []` (`contract/cli/src/tamber_params.rs:510`, `docs/tamber-protocol.md:481`). A bool’s options are `"false"` then `"true"` (`contract/lower/src/anatomy.rs:737`). The panel copies that list in order and prints one text line per prop (`app.contract:1867`). That block is only `text` nodes: no `select`, `button`, or `input`, and no option is marked as the one on screen.

A component with variants and an empty `params` array is included (`components.ts:143`, `c.params ?? []` at `components.ts:147`) and covered by the last contract test: Chip’s enum and bool, Badge with variants only, and no variants block on Card. The stub’s `/__stub/library` path adds that case as `params: []` (`stub/tamber-stub.ts:313`).

The other component walks still see a `params` array, which the wire always has. `paramInfoOf` (`rescope.ts:20`), the level-name collection (`app.ts:121`), the kinds map (`app.ts:732`), the dials panel, and the branch `param` flag (`app.ts:781`) never read variant props, so a variants-only entry does not become a dial, a level, or a branch value.

Read-only review; no files changed, builds, or test suites run.
