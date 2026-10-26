# Code review: the variant gallery's G1, variant props in the component panel, round 1 of 2, 2026-10-20 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `369e6e24078733995f6c79e219320ba2e791bd58c29974d9a5030838ff15b6f0`), shared by both reviewers, each blind to the other's review; HEAD `c3221e7241`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (1 NIT).
- **Disposition:**
  - Astra 1: the heading no longer names an LLP or promises pins: "Variants: the options each prop takes (the tool pins none on an instance)".
  - Astra 2: the test's comment now claims only what its assertion checks.
  - Grok 1: the Design.components comment names variant props too.

1. **NIT** — `apps/design-tool/app.contract:526` still says the panel list is "Each component with params." `componentsOf` now also returns a component that has variants and an empty `params` array, and that entry is what the new test renders for Badge. The comment on `variants` just above is right; this one describes the old membership.

The variant listing matches stage 12 D3. `components()` emits `variants: [{prop, options}]` with string options (`contract/cli/src/tamber_params.rs:520`, `docs/tamber-protocol.md:481`); a bool is `"false"` then `"true"` (`contract/lower/src/anatomy.rs:737`). The panel copies that list in order and prints one text line per prop (`app.contract:1867`). There is no `select`, `button`, or `input`, and no option is marked as the one on screen.

A component with variants and no params is included (`components.ts:143`, `c.params ?? []` at `components.ts:147`) and covered by the last contract test: Chip's enum and bool, Badge with variants only, and no variants block on Card. The stub's `/__stub/library` path adds that case as `params: []`.

The other component walks still assume a `params` array, and the wire always has one (`[]` when there are no params). `paramInfoOf` (`rescope.ts:20`), the level-name collection (`app.ts:121`), the kinds map (`app.ts:732`), the dials panel, and the branch `param` flag (`app.ts:781`) never read variant props, so a variants-only entry does not become a dial, a level, or a branch value.
