# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `53c45e6877`.
- **Method:** one brief (sha256 `b131f7b0091e86399a2210937d240ed8cf486d9d65fdfdc1cd9b6619b43f7ca1`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

1. **P2 — word/span mismatch remains** at [contract/lower/src/theme/scan.rs:384](contract/lower/src/theme/scan.rs:384). `Grouped::Swipe` propagates to every descendant, suppressing marked wrappers’ looks too. Build lowering preserves wrappers and styles only the named content node ([swipe_rows.rs:338](contract/lower/src/swipe_rows.rs:338)). Reproducer: wrap the swipe test’s content and action in `row -exact-row-look="platform" padding-left="gap" margin-left="gap"`, then change `gap` from `8px` to `#444444`. The scan reports `margin-left`; the build first reports `padding-left`, with different words and column. **Fix:** share the build’s swipe eligibility and content-selection logic, suppress only the selected node’s look, and add a `same_words` regression for the wrapper.

This still blocks approval. The audio, plain-section, and build-gate fixes look correct. Reviewed both commits read-only; tests were not rerun.
