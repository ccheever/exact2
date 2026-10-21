# Code review r2: a landed flight's view takes its clip and corners from its style (LLP 1013.000 D4/D4.4), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x2fr-rv`.
- **Method:** one brief (sha256 `1296861248e77db8d2e09d67decb28d4e2111e42dc42b4fee87ffe237640a148`), shared with the other reviewer. Round 2, blind to the other's round-2 review; both round-1 reviews and dispositions available. Reviewed commit 1badc497c in a detached worktree. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition (r3):** no findings. After this round the macOS `overflowClips` was rewritten shorter, with the same predicate, to keep NodeViewMac.swift under the 1,500-line cap; round 3 confirms that rewrite only.

---

**READY**

No findings requiring changes.

The round-1 correctness issues are resolved: iOS restores corners after final geometry and invalidates display to refresh the gradient; both hosts derive landing clipping from current style. macOS’s box pass assigns a positive backing-layer radius only when clipping is intended.

The `overflowClips` extraction preserves existing style-pass behavior, including macOS paragraph overflow and line-clamp, iOS dormant scrolling, and clip-box handling. I found no additional regressions in the changed paths.

Validation: reviewed HEAD and the round-1 amendment; clipping-predicate equivalence and `git diff --check` passed. Native runtime verification was not completed here; XCTest evidence is from your supplied results. No files changed.
