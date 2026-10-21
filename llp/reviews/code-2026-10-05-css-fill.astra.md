# Code review: an ended CSS animation without a forwards fill stops holding its property (Apple, agent clock), 2026-10-05 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/x20`.
- **Method:** one brief (sha256 `180e9494b3304c9914ba6a36bcd81ec3f7a19dd7cebbd14ee27ee5e65e8d61db`), shared with grok. Round 1, blind to the other review. Reviewed the uncommitted diff in a worktree at origin/main. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** The optional test improvement taken: a clock-only `apply` (no `h`) to the end exactly and back.

---

**READY** — no blocking findings against `origin/main`.

- [SvgScene.swift:177](/tmp/x20/host/apple/Sources/ExactKit/SvgScene.swift:177): Correctly removes `none`/`backwards` effects at the finite end, including fractional iteration counts. `forwards`/`both` retain the existing endpoint clamp; `n < 0` remains infinite, and backwards fill during a delay is unchanged. This matches [CSS fill semantics](https://www.w3.org/TR/css-animations-1/#animation-fill-mode).
- [svg_lower.rs:204](/tmp/x20/host/apple/src/svg_lower.rs:204): Direction is already lowered. Alternate doubles the period and halves the repeat count, preserving total duration and the appropriate final position.
- [SvgScene.swift:94](/tmp/x20/host/apple/Sources/ExactKit/SvgScene.swift:94): Cache interaction is sound. Clock changes alter the signature; `h` participates in the spec hash. `apply` removes the previous animation before `make` returns nil, and seeking backward reinstalls it.
- [SvgFilterLive.swift:306](/tmp/x20/host/apple/Sources/ExactKit/SvgFilterLive.swift:306): Offscreen rendering forces a rebuild; the guard runs before offscreen timing setup. No production caller bypasses `apply` or requires the erroneous hold. [macOS uses the same implementation](/tmp/x20/host/apple/Sources/ExactKit/Mac/PresenterMac.swift:998).

The [new tests](/tmp/x20/host/apple/tests/ExactKitTests/CssAnimationFillIOSTests.swift:16) cover the core regression. Optional improvement: omit `h` in the `apply` test, then seek to exactly 200 ms and back to 100 ms. Currently both `h` and clock change, so clock-only cache invalidation is not isolated.

`git diff --check` passed. I did not rerun XCTest; inspecting the existing result bundle was blocked because `xcresulttool` requires temporary writes. The reported 281 passing tests remain author-provided evidence.
