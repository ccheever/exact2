# Code review: The write check judges a style's rows where an element applies it (stage 13 D QUEUE), round 2, 2026-10-22 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `fd7aa4d783`.
- **Method:** one brief (sha256 `9833135a2c6fbf38e337f4f3f98c61e8f1c7304b16c5a29108b7012d2a175698`), blind to the other reviewer; 3 rounds (theme2's 02:08 ruling), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

- **P2 — Class rows overridden by `delete=` expansion are still judged.** [scan.rs:485](contract/lower/src/theme/scan.rs:485) applies `class::rows` to the original attributes. For a normal cell list containing `row class=S delete=remove`, with `S` declaring `width="gap"`, changing `gap` from `8px` to a colour causes a false refusal. The build’s [swipe expansion](contract/lower/src/swipe_rows.rs:112) inserts `width="100%"` before class merging, which removes that class width entirely. The same applies to its injected `flex-shrink`, `scroll-snap-align`, and `box-sizing`. **Fix:** account for the cell-row expansion before collecting class rows, or leave affected class rows to the build. Add a regression asserting the write and resulting build both succeed.

- **P3 — Expanded identity and ordering lack a scan-level regression.** [rows.rs:827](contract/lower/src/theme/rows.rs:827) assigns element numbers manually; the two-element integration test produces identical diagnostics. Neither exercises the repeated-component span collision or verifies `order()` chooses between different elements’ words. **Fix:** obtain retained rows from an actual twice-inlined component with different font contexts, then check per-instance before/after eligibility and the first element’s diagnostic.

Preorder itself matches lowering’s branch and expanded-component traversal; I found no additional ordering defect.

Tests could not run: the read-only sandbox blocked Cargo’s build-directory creation. No files edited.
