# Code review r3: a list's rows moved by the list play no layout-transition (66f3b8b51..0b0e34c8f), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `0b0e34c8f`.
- **Method:** one brief (sha256 `09b07dcfcb768e6985705b90d2f172f0bed3e72b5e4601b6a3a6f833ec9e1220`), shared with grok. Round 3, the last, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** DO NOT LAND.
- **Disposition (landed):** 1 taken: a wrapper that moves across its list (its x in a vertical list, its y in a row list; `collection_data` now carries the axis) un-settles the list, as a padding swap does; the test swaps the list's left and right padding and fails without the check. 2 taken: places and sizes are compared to a hundredth of a point, since they are rebuilt from absolute `f32` coordinates; a test with a fractional margin across a correction is DEFERRED. 3 taken: the nudge is a fixed-width row moved by `position: relative` `left`, and it fails without the box check; same-size body updates with corrections, nested lists and synchronous edge actions stay DEFERRED. 4 taken: with no layout transition the judgment keeps only the generations and returns before matching.

---

DO NOT LAND

1. **MATERIAL — Authored container offsets still snap.** [presence.rs:205](host/apple/src/presence.rs:205) ignores wrapper position changes. Swap a vertical list’s left/right padding from `0/20` to `20/0`: keys, wrapper sizes and row-relative frames stay unchanged, but every row moves 20px and loses its transition. Cross-axis padding is allowed. **Fix:** distinguish authored container offsets from collection corrections; add this regression.

2. **MATERIAL — Fractional margins defeat the correction fix.** [presence.rs:215](host/apple/src/presence.rs:215) compares relative frames reconstructed from absolute `f32` coordinates ([host.rs:1353](host/apple/src/host.rs:1353)). With unchanged `margin-top=0.1`, moving a wrapper from y=2040 to 2080 changes the reconstructed margin from `0.099975586` to `0.100097656`—verified with float32 arithmetic. That unsets settlement for the entire list, animating the 40px correction. **Fix:** compare stable local geometry before absolute-coordinate rounding; test fractional margins across such a correction.

3. **MINOR — The nudge test does not isolate position changes.** [collection_tests.rs:504](host/apple/src/collection_tests.rs:504) changes `margin-left` on an auto-width stretched row, also shrinking its width. Size-only detection can pass. **Fix:** use a fixed-width row with relative `left`, and assert intermediate presentation. Same-size body updates combined with corrections, nested-list churn and synchronous edge-action animation coverage remain missing.

4. **MINOR — No-transition layouts still pay for classification.** [presence.rs:180](host/apple/src/presence.rs:180) traverses collections, rebuilds the generation map and populates `settled` before checking whether transitions exist. **Fix:** retain the generation baseline but return before matching when empty; measure the remaining traversal cost.

Earlier key-change, mixed-row growth, report-scoping and boot-baseline fixes hold. No additional race or lifetime defect found. Formatting, `caps`, `boot` and diff checks pass; source files meet the limit. Rust tests and app drives were not run: this read-only checkout has no build artifacts.