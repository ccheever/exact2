# Code review: a segmented tablist is measured only when what sizes it changes (3adba3171), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `3adba3171`.
- **Method:** one brief (sha256 `629bc35622c922396527c1081f2eab719d887984dff8a927b6fcab63a9654625`), shared with grok. Round 1, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition (r2):** 1 taken conservatively: a control with a background or divider image (a hook's own look) is measured every time and its size is not cached. 2 taken in part: `testASegmentedTablistIsMeasuredAgainWhenWhatSizesItChanges` installs and removes a hook's background image and raises the text size, comparing each report with a fresh `intrinsicContentSize` (it fails with a cache that never invalidates). No iPad size-class or tvOS test.

---

LAND WITH CHANGES

1. **MATERIAL — Native appearance changes can leave the minimum height stale.** [SegmentsIOS.swift:205](host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:205) omits background images and bar metrics. A supported element hook receives the control [after measurement](host/apple/Sources/ExactKit/ElementHooks.swift:52) and can install a custom background via [UIKit’s appearance API](https://developer.apple.com/documentation/uikit/uisegmentedcontrol/setbackgroundimage(_:for:barmetrics:)). Switching its native sizing appearance leaves this key unchanged: subsequent batches reuse the old height, which remains the kernel’s [automatic minimum](kernel/src/style.rs:1429). Previously, the next pass remeasured. **Fix:** invalidate the cache through UIKit’s intrinsic-size invalidation and relevant trait changes, or conservatively bypass it for customized controls. Test background installation/removal after the first measurement.

2. **MINOR — Existing tests do not exercise cache invalidation or avoided measurement.** [PresenceIOSTests.swift:117](host/apple/tests/ExactKitTests/PresenceIOSTests.swift:117) counts published reports, already deduplicated by `sizes`; both unconditional measurement and permanently caching the first natural size pass. No tests changed. **Fix:** assert measurement counts and compare subsequent reports with fresh UIKit measurements after title, image-dimension, font, native-appearance and trait changes. Include iPad size-class and tvOS coverage, plus replacement/reset.

The tab-bar measurement branch, border-box adjustments and generation guards remain unchanged. Earlier idle-tick dispositions remain implemented, including [projection ordering and geometry replay](host/apple/Sources/ExactKit/IOS/PresenterIOS.swift:24). No earlier segment-cache reviews were present. The changed file is 408 lines; no apparatus or documentation change is required.

Static review only; UIKit tests were not run in this read-only checkout.