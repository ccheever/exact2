# Code review: a column flex item's main size measured at its fit-content width (Taffy patch 28; numbered 27 when reviewed, renumbered on landing after a multicol patch took 27), round 3, 2026-10-05 (grok)

- **Family:** xAI. `grok-4.7` via the grok CLI, reasoning effort xhigh, `--no-subagents`, `--cwd /tmp/x26`.
- **Method:** one brief (sha256 `2fa104dabe8257939d3213485f3be80977706d4c477ea723755fe10c199abe05`), shared with astra. Round 3 (the last), blind to the other review. Reviewed the staged diff in a worktree at 39955bf5d. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** Round 3 was the last. Astra's one finding (P3: the comment in flexbox.rs and the patch note still wrote the formula in the old order) is taken: both now read max(min-content, min(max-content, available)). A comment and doc change only, after round 3, not reviewed again. Grok: READY, no findings.

---

READY

No findings.

Round 2 is fixed. `determine_flex_base_size` and `determine_hypothetical_cross_size` both call `common::fit_content_width`, and that helper now uses CSS Sizing 3’s `max(min-content, min(max-content, available))` (`common/mod.rs:33`). When min-content exceeds max-content, the result is min-content. The old `min(max(available, min-content), max-content)` returned max-content. For the staged negative-margin row (min-content 100, max-content 60, available 80) both passes now yield 100, which is the width the new test asserts.

The same order is right for the other two callers. It only changes results when the intrinsic sizes are inverted, and in that case it matches the Sizing 3 fit-content size those callers already implement:

- Grid automatic inline size (`grid/alignment.rs:281`) passes `Size::NONE`, both definiteness flags true, and `SizingMode::InherentSize`.
- Absolute shrink-to-fit (`common/absolute.rs:291`) does the same. Sizing 3 defines shrink-to-fit as this fit-content size, and floors max-content by min-content, so the CSS 2.1 spelling agrees once the sizes are inverted.

The probe and the final column-item width use the same condition for this path: column, `width: auto`, no aspect ratio, not a compressible replaced element. The extra post-clamp (min/max, then padding plus border) matches the final width numerically, including when the available width is clamped or floored first. Definiteness flags on the probe call are `Size { width: true, height: true }` with both known axes empty (`flexbox.rs:1064`), which is the same pattern grid and absolute use. Empty axes are normalized to definite in the cache key and in flex layout, so the flags do not change the intrinsic widths. The later height measure goes through `measure_child_size`, which marks the known fit-content width definite, matching `item_known_dimension_definiteness` for a column whose cross available space is definite.
