# Code review: the variant gallery's G1, variant props in the component panel, round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `bcdfb0ca8541a7c7eb61fdf690d2dfbc13fdc41701882599440767a1362d5c1c`), shared by both reviewers, each blind to the other's review; HEAD `debe99a923`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks; round 1's three NITs closed, no new findings.
- **Disposition:**
  - none: round 2 had no findings from either reviewer.

Nothing blocks G1. No new findings. All three round-one nits are closed:

1. **NIT — closed — [app.contract:1866](/tmp/rv-dt-g1r2/apps/design-tool/app.contract:1866):** The heading removes the LLP reference and promise of future pins.
2. **NIT — closed — [app.test.contract:2208](/tmp/rv-dt-g1r2/apps/design-tool/app.test.contract:2208):** The comment now limits its claim to Badge’s missing param group.
3. **NIT — closed — [app.contract:526](/tmp/rv-dt-g1r2/apps/design-tool/app.contract:526):** The component-list comment includes variant props.

The full change matches `describe`’s `{prop, options}` shape and `params: []` guarantee. Variant rows are plain text, with no editable control or selected-value claim. Empty params remain safe through the panel loops, `paramInfoOf`, levels, dials, rescope, and branches. The final test covers enum and bool options, Badge’s variants-only entry through `/__stub/library`, and Card’s absent variants block.

Read-only review; no files changed, builds, or test suites run.
