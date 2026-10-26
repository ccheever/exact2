# Code review: the variant gallery's G1, variant props in the component panel, round 1 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `369e6e24078733995f6c79e219320ba2e791bd58c29974d9a5030838ff15b6f0`), shared by both reviewers, each blind to the other's review; HEAD `c3221e7241`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (2 NITs).
- **Disposition:**
  - Astra 1: the heading no longer names an LLP or promises pins: "Variants: the options each prop takes (the tool pins none on an instance)".
  - Astra 2: the test's comment now claims only what its assertion checks.
  - Grok 1: the Design.components comment names variant props too.

Nothing blocks G1. The describe shape matches, including `params: []`. Empty params remain safe through the panel, `paramInfoOf`, levels, dials, rescope, and branches. Variant rows are plain text with no selected value; Badge’s entry is covered.

1. **NIT — [app.contract:1866](/tmp/rv-dt-g1r1/apps/design-tool/app.contract:1866):** The visible heading exposes an internal LLP reference and promises pins “come later,” although Q5 makes them conditional on demand. “Variants (available options)” would describe the current feature clearly.

2. **NIT — [app.test.contract:2208](/tmp/rv-dt-g1r1/apps/design-tool/app.test.contract:2208):** The “No control” assertion only checks that Badge lacks an unnamed param group. The text assertions would also accept buttons containing those labels, and wouldn’t catch a separate selected-value indicator. Strengthen the assertion or narrow the comment’s coverage claim.

Read-only review; no files changed, builds, or test suites run.
