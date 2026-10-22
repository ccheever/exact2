# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `af82d07cbf`.
- **Method:** one brief (sha256 `fe1819ef4c9ce1dcde07e4951c3b3661cb41f4abef7d37b0cc5e7c126bec69cb`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Items 1–2 still block approval.

1. **Blocker — [contract/lower/src/theme/scan.rs:273](contract/lower/src/theme/scan.rs:273): audio transformation misses class rows.** With `style Hidden` setting `display="none"` and `audio "https://example.com/a.mp3" class=Hidden controls=on`, the scan generates `on ? "block" : "none"`. Lowering expands the class first and generates `on ? "none" : "none"`. Adding `block = 4px` therefore makes the server refuse a write the build accepts. **Fix:** pass the effective class-expanded attributes to `audio_rows`, preserving their spans; add this bound-controls regression.

2. **Major — [contract/lower/src/theme/scan.rs:268](contract/lower/src/theme/scan.rs:268): a plain section still suppresses looks.** Put `column -exact-row-look="platform" padding-left="gap" margin-left="gap"` inside an ordinary `section`, outside a grouped list. Changing `gap` from `8px` to a colour makes the server report `margin-left`; the build reports `padding-left` first, with different words and column. **Fix:** track actual grouped-list section context, using `grouped::style` over effective attributes, rather than the parent tag alone. Add a `same_words` regression for a plain section.

3. **Minor — [contract/cli/tests/it/tamber_rows_check.rs:1029](contract/cli/tests/it/tamber_rows_check.rs:1029): “before verdict” is not guaranteed.** The background failure can finish and be polled before the `wait=false` transaction, so both branches can exercise the failed-build gate. **Fix:** hold that branch’s worker with the existing `EXACT_TAMBER_CHECK_GATE` until the fixing transaction succeeds.

The `Row::within` propagation/grouping and explicit retention-key plumbing look correct.

Static review only: no build artifacts were available, and the read-only sandbox prevented running the regressions. No files changed.
