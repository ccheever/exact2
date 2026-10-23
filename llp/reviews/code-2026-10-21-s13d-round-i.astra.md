# Code review: Tamber stage 13 D, the write check's gaps (LLP 1121), narrow re-check, 2026-10-21 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `df08adcb4a`.
- **Method:** one brief (sha256 `89abdf91e1dedb0a939148db0d8c891b4fdbbf93f3feb222b90cb2b0f93b1a14`), blind to the other reviewer; D1 and D3 3 rounds (they touch lowering's shared code), D2 2 rounds (theme2's 15:31 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

1. **P2 — [contract/lower/src/theme/scan.rs:322](contract/lower/src/theme/scan.rs:322): span equality still misidentifies a real descendant as rebuilt.** `written()` passes original descendant references to `row`, including through elements, `if`, and `match`. Two nested uses of a slotted scroll component share their root span. If the outer scroll’s `swipeContent` names the inner scroll, this fallback records the outer address instead.

   Concrete case: both roots have `-exact-row-look="platform"` and `padding-right=pad`; pass `"0px"` outside and `"gap"` inside. Changing `gap` from `8px` to `#444444` falsely refuses the write: the scanner checks the inner padding, although lowering drops it.

   **Fix:** pass explicit original-node identity through the `parts()` path, or distinguish original-tree membership by address before remapping. Do not infer rebuilding from span equality. Add this nested-scroll regression alongside the existing tests.

**One blocker remains.** Addresses are stable during the immutable walk, and extending the inherited set is correct. No files changed; tests were not rerun because this read-only checkout has no build artifacts.
