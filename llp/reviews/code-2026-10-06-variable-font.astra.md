# Code review: a declared variable face draws at its declared weight (Apple), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C /tmp/bsky-vf-rv`.
- **Method:** one brief (sha256 `cb93ab1836a509b29cc75beaebf3623ff14bbed1d3045d10c6ef7684a5bb0cc5`), shared with the other reviewer. Round 1, blind to the other review. Reviewed commit 3c6afedba in a detached worktree. The author (Claude, for the Bluesky clone) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** READY.
- **Disposition:** 1 taken: the test now also installs a two-face catalog (one variable file at 400 and 600) through `TextEngine.install` and reads the drawn `wght` from `font(size:weight:family:italic:)`; it fails with the registration call removed (checked by mutation). The test runs in the macOS Swift lane, which is where `TextCSSTests` lives.

1. **P3 — Nonblocking test coverage gap — [TextCSSTests.swift:152](/tmp/bsky-vf-rv/host/apple/tests/ExactKitTests/TextCSSTests.swift:152).** The test catches the original Thin-instance failure, but calls `declaredFace` directly: removing its registration call would leave this test green. It also never runs in the normal iOS lane, which selects only `*IOSTests` classes ([build.mjs:1469](/tmp/bsky-vf-rv/host/apple/build.mjs:1469)). Suggested improvement: add a catalog-install → `font(...)` regression asserting resolved identity and variation at 400/600, and exercise it on iOS.

No production correctness defects found. Registration is the right place: each declared row represents a fixed-weight face. Matching should continue using the **declared** weight, while its descriptor uses the axis-clamped weight. This agrees with the web host’s individual `@font-face` rules and [CSS’s descriptor-clamping rule](https://www.w3.org/TR/css-fonts-4/#feature-variation-precedence), including mixed static/variable families and the single-path 400 declaration.

Read-only macOS CoreText probes confirmed that URL and data descriptors resolve correctly, including non-named weights; italic traits survive; lower and upper clamping work; and cascade/feature copies retain the variation. `opsz` remains size-dependent—the temporary 16-point font does not freeze it. Existing cache invalidation is sufficient, and declared faces return before the system-only `cssWeight` path.

I did not build or run XCTest or iOS under the read-only constraint.

READY
