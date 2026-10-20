# Code review: LLP 1121 stage 12 B2, expressions over base names per host column, round 2 of 3, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `0aec848924`.
- **Method:** one brief (sha256 `8808f3454bdd0a04c7dd61c9a84fc4c6200453f35c6deeea3c672da25690f5a3`), blind to the other reviewer; the commit as amended after the previous pass; 3 rounds (a kernel and compiler change, theme2's 00:42 ruling), then narrow checks of the last fixes. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.
- **Verdict:** approve with changes (2 SHOULD-FIX, 1 NIT).
- **Disposition:** each taken in the next amend ("Round 2").

No BLOCKING findings. Two SHOULD-FIX issues and one NIT:

- **SHOULD-FIX — [kernel/src/style/theme.rs:269](kernel/src/style/theme.rs:269), [contract/lower/src/scale.rs:72](contract/lower/src/scale.rs:72): folded px can fail to parse back.** `pad = calc(space.md * 1e38)` compiles when selected through state into `padding`. Evaluation saturates to `f32::MAX`, but its printed decimal exceeds the exact maximum and `parse_pixel_length` rejects it. Static use already refuses the same token. The `unfolded` guard therefore still misses a computed-use failure. Ensure the serialized result is readable, with a regression covering this boundary.

- **SHOULD-FIX — [tamber/src/residual.rs:173](tamber/src/residual.rs:173): exports evaluate a different arithmetic order.** `fold()` adds the constant first; the carried CSS puts it last. For `calc(space.md * 1e16 - space.sm * 2e16 + 1px)`, neutral-column evaluation yields **1px**, while both exporters produce **0px** through this method. It also lacks the kernel’s handling of nonfinite results. Export folding should match the carried expression’s evaluation.

- **NIT — [tamber/src/shadow.rs:292](tamber/src/shadow.rs:292): the shadow diagnostic fix misses aliases inside functions.** With `lift = space.md`, `0 calc(lift * 2) 4px black` reports “not a colour” and advice about type tokens, without mentioning shadows. Direct bases and bare aliases now diagnose correctly; this scan needs to follow aliases too.

The other Round 1 fixes check out: parser-limit refusal, fractional bounds compared at f32 precision, finite arithmetic checks, coefficient division without reciprocals, underflow removal, and cancelled-base bookkeeping. I found no additional regression in kind inference or `Scope::base` recording.

The kernel lookup/read path, recursion guard, terminal cells→px→layout rounding, written-tuple inheritance rule, and JS properties/SVG/release-versus-dev paths look consistent with the design. No schema, digest or wire change. The conformance fixture’s column values are correct.

Validation: **10 selected prebuilt tests passed**, compiler stdin probes reproduced the findings, and **43 saved before/after plans were byte-identical**. No files changed. Browser/native runs were not repeated. Tests still miss these findings, live edits defining a previously unset base, and fractional terminal rounding.
