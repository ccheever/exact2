# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), D3 round 3, D1/D2 narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `700c41d3d0`.
- **Method:** one brief (sha256 `d1e539c15ec30edc73d4671d0cc6ca22c6dea8d1aa0340a15782e3dc5057fe24`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Two issues remain; **finding 1 blocks approval**.

1. **Blocker — D2 — [contract/lower/src/theme/scan.rs:252](contract/lower/src/theme/scan.rs:252): the check retains an audio `display` that lowering discards.**

   With no `block` token initially:
   ```text
   component App
     state on = false
     state pick = "block"
     view
       audio "https://example.com/a.mp3" display=(on ? "block" : pick)
   ```
   Adding `{"op":"add-token","name":"block","value":"4px"}` makes the checker report `tamber-kind` against the literal arm. However, [media.rs:124](contract/lower/src/media.rs:124) removes authored `display` when controls are absent and inserts `"none"`. The build accepts both designs. The failed-build gate cannot help.

   **Fix:** leave audio `display` rows unjudged, or apply the shared audio rewrite before retaining them. Add a regression asserting successful builds before and after the accepted transaction.

2. **Major — D3 — [contract/lower/src/theme/rows.rs:652](contract/lower/src/theme/rows.rs:652): a newly failing font preflight does not exclude descendant rows.**

   Start with `family.text: family = system-ui`, `pos = 8px`, and:
   ```text
   component App
     view
       svg width=20 height=20 font-family="family.text"
         rect x="pos" width=10 height=10
   ```
   In one transaction, remove `family.text` and set `pos` to `#444444`. The original file builds. Afterwards, the build stops at the SVG’s `font_use`: the now-literal `family.text` produces `lower-font-family-list`. It never lowers the rectangle. The write check instead reports `tamber-kind` on `x`, because its font preflight guard covers only font rows.

   **Fix:** retain element/ancestor font preflight context and run shared `font_use` against the candidate design. If it fails, leave that subtree’s rows to the build. Cover this two-operation transaction with a regression.

The two fixes in `700c41d3d0` otherwise look correct. `row_look::look` preserves the lowering predicate. `synthetic()` installs `last_key` before polling and before `write_checks`; I found no harmful key mismatch, including verified scrubs. A retained failure suppresses checks conservatively and keeps classification at `rebuild`, consistent with allowing missed refusals. No separate D1 blocker found.

This was a read-only source review. I did not execute these fixtures, run tests, or independently verify the 48-plan byte comparison.
