# Code review: a followed end's measured row is its motion's one target; a port at a device pixel is at its target, round 3 (00899a8f4), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `00899a8f4`.
- **Method:** one brief (sha256 `f37606037bbbcc4551c13263e00ae22e333c44ec014ac9947e58e59c74aae568`), shared with grok. Round 3, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: the folds go to 900.49 and 0.49 (still under half a point) and assert the port moved past 900.2 and 0.2, which a fold that skipped its setter cannot.

---

LAND WITH CHANGES

1. **MINOR — Folding tests still pass without setting the offset.** [SmoothCollectionIOSTests.swift:220](host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:220) accepts unchanged `900` for `900.2 ± 0.34`; [line 234](host/apple/tests/ExactKitTests/SmoothCollectionIOSTests.swift:234) likewise accepts unchanged `0`. Removing either folding-path assignment leaves these assertions green, so round 2’s test disposition is incomplete. **Fix:** use a `0.4`-point fold and assert actual movement on the tested 2×/3× screen, or explicitly record the setter call.

No material regression found; the earlier runner and idle-tick correctness fixes remain intact. Whitespace, source-size checks, and 20 JS method-probe assertions passed. Rust/UIKit suites and browser conformance were not run in this read-only checkout.