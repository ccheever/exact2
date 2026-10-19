# Code review: Stage 11 C harness scripts/scale-check.mjs, round 2, 2026-10-19 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `cbb9fb64aa`.
- **Method:** one brief (sha256 `160f922579d65350dc9c413ecf152bd25715b9a92694d1764377804424e8e00c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** one P1 (a covered middle-row reference could hide a wrong radius), three P2s
- **Disposition:** all taken in 517db7ac60: the reference must be at the box's left edge; a radius no row read can show is the same reading as square; a child must be named; an expectation must be a number

Blocking issues remain.

1. **[P1] A displaced middle-row reference can hide a wrong radius** — [scale-check.mjs:73](/tmp/rv-s11c-2/scripts/scale-check.mjs:73). Consider a red box with radius 12 whose middle row’s first 12 pixels are covered by white background. `start()` accepts that later red edge; subtracting it and clamping makes every corner inset zero. An expectation of radius 0 then passes. Validate that the reference is near the box’s geometric left edge, and reject a displaced reference. Add a background-coloured overlap/clipping regression.

2. **[P2] Small radii at fractional Y origins can exceed the promised 1-point error** — [scale-check.mjs:51](/tmp/rv-s11c-2/scripts/scale-check.mjs:51). At 1×, the test helper’s box at `(10, 10.01)` with radius `1.2` has solid left-edge pixels in all six sampled rows. The first row’s centre is already 1.49 pixels below the top. Consequently, the least-radius rule returns 0 and rejects the correct radius by 1.2 points. Preserve information from the partial top row or account for this ambiguity when comparing. Existing tests jump from radius 0 to 4.

3. **[P2] Omitting `child` can still pass padding validation** — [scale-check.mjs:118](/tmp/rv-s11c-2/scripts/scale-check.mjs:118). When `want.child` is undefined, the lookup matches the first node without a `testId`. An untagged node at offsets `(16,16)` therefore passes a padding-16 expectation without an explicit child. Require a nonempty child identifier before searching. The current missing-child test misses this because every fixture node is tagged.

4. **[P2] Nonnumeric expectations now pass silently** — [padding comparison](/tmp/rv-s11c-2/scripts/scale-check.mjs:123), [radius comparison](/tmp/rv-s11c-2/scripts/scale-check.mjs:136). Values such as `"16px"` produce `NaN`; both new `difference > tolerance` checks then evaluate false, leaving `ok: true`. Require finite numeric expectations and measured geometry. This regresses the previous positive acceptance comparisons.

The unobscured pill midpoint and `x === left` formula look reasonable; off-image boxes, including taller-than-screenshot boxes, now fail. macOS’s relevant [capture path already draws in sRGB](/tmp/rv-s11c-2/host/apple/Sources/ExactKit/Mac/AgentMac.swift:1231), addressing the display-profile concern. The projection and span thresholds look reasonable for the specified isolated probes, though tests cover only ideal colour blends. No further blocker found in host continuation, storage reset, CLI fixes, or QUEUE wording.

Read-only review; no builds or tests run.
