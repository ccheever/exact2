# Code review: a column flex item's main size measured at its fit-content width (Taffy patch 28; numbered 27 when reviewed, renumbered on landing after a multicol patch took 27), round 3, 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x26`.
- **Method:** one brief (sha256 `2fa104dabe8257939d3213485f3be80977706d4c477ea723755fe10c199abe05`), shared with grok. Round 3 (the last), blind to the other review. Reviewed the staged diff in a worktree at 39955bf5d. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY WITH CHANGES.
- **Disposition:** Round 3 was the last. Astra's one finding (P3: the comment in flexbox.rs and the patch note still wrote the formula in the old order) is taken: both now read max(min-content, min(max-content, available)). A comment and doc change only, after round 3, not reviewed again. Grok: READY, no findings.

---

READY WITH CHANGES

1. **P3 — Incorrect formula in comments/documentation:** [flexbox.rs:1032](/tmp/x26/vendor/taffy/src/compute/flexbox.rs:1032) and [EXACT-PATCHES.md:842](/tmp/x26/vendor/taffy/EXACT-PATCHES.md:842) still state `min(max-content, max(min-content, available))`. For the new regression’s intrinsic widths, that yields 60 instead of 100. Replace both with `max(min-content, min(max-content, available))`, matching the implementation and [CSS Sizing 3](https://www.w3.org/TR/css-sizing-3/#fit-content-size).

No runtime correctness findings. Round 2’s mismatch is resolved; the helper’s order is correct for all four callers, including grid and absolute positioning. The probe’s clamp, padding floor, and definiteness flags are consistent.

Validation: the existing executable passed **36 browser-fixture tests and 14 layout-equality tests**. No rebuild or file changes.
exit 0
