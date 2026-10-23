# Code review: the painted popover's default placement and AppKit live margins, round 2, 2026-10-23 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, read-only sandbox, a throwaway worktree at `afd265f7b2`.
- **Method:** one brief (sha256 `5d00c62318d2409fd3ed0744072b4fe66a6f026050f471e720cdc53b8c2aed2c`). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** WRONG
- **Disposition:** taken in 9deccf3a62 (popover margins cross)

**P2 — The new default placement loses authored margins.** [PositionArea.swift:25](/tmp/iosfix-rv-flip2/host/apple/Sources/ExactKit/PositionArea.swift:25) synthesizes the area only in Swift, but [style.rs:1057](/tmp/iosfix-rv-flip2/host/apple/src/style.rs:1057) sends margins only when the kernel’s `position_area` is non-`none`. Consequently, `PositionArea.margins(of:)` receives zeros for these default popovers.

Concrete case: bounds `400×600`, anchor `(12,380,30,30)`, popover `300×180`, authored `margin: 12px 0 0`, no authored area. Below, the margin box ends at `602`, so CSS selects `flip-block`, putting the border box at **`(12,188)`**. Apple ignores the margin and accepts below at **`(12,410)`**. CSS tests the margin box and flips the corresponding margins. [CSS Anchor Positioning](https://drafts.csswg.org/css-anchor-position-1/#fallback)

Carry authored margins through the encoder for the newly defaulted painted path. The assertion at [PositionAreaTests.swift:61](/tmp/iosfix-rv-flip2/host/apple/tests/ExactKitTests/PositionAreaTests.swift:61) supplies margins directly, bypassing this failure.

The remaining scoped changes look right:

- `origin` is identical to main. The chooser still uses `of`: absent-area `.platform` placement retains its old clamp; authored-area no-fit placement remains unshifted.
- Default option order, supplied-margin flips, RTL spanning, and retention of a fitting/current option across bounds changes are correct. Existing size-change invalidation remains unchanged.
- Authored areas and fallback lists—including explicit `none`—remain distinct from absence. The fixed/absolute overflow distinction and iOS keyboard clamp remain intact.
- The new opening-position assertions distinguish flipping from the old clamp. QUEUE’s approximately 20% figure matches the reported standard-deviation ratios; I did not reproduce its measurements.

No files changed. XCTest was not run.
