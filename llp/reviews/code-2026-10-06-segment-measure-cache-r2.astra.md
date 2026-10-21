# Code review: a segmented tablist is measured only when what sizes it changes, round 2 (9dc40108e), 2026-10-06 (astra)

- **Family:** OpenAI. `gpt-6-astra` via `codex exec`, reasoning effort xhigh, sandbox read-only, `-C` a detached worktree at `9dc40108e`.
- **Method:** one brief (sha256 `04a7497ddb82afbc2afc1eccb3fff967f36967f2492b74bf394ba49fbc72fe43`), shared with grok. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** LAND WITH CHANGES.
- **Disposition:** 1 taken: each segment's explicit width and `apportionsSegmentWidthsByContent` are in the key. 2 not taken: no measurement-count or title-change test.

---

LAND WITH CHANGES

1. **MINOR — Native width configuration is missing from the key.** [SegmentsIOS.swift:205](host/apple/Sources/ExactKit/IOS/SegmentsIOS.swift:205) omits explicit segment widths and `apportionsSegmentWidthsByContent`. A supported hook can change these after measurement; the next sync retains the cached width, whereas the old pass remeasured. UIKit exposes these as [sizing configuration](https://developer.apple.com/documentation/uikit/uisegmentedcontrol). Current tablist layout consumes only height ([style.rs:1429](kernel/src/style.rs:1429)), so this is a stale measurement rather than an established visible regression. **Fix:** include those inputs, or explicitly cache only the height the seam requires.

2. **MINOR — The new test still does not pin cache invalidation or avoided measurement.** [PresenceIOSTests.swift:97](host/apple/tests/ExactKitTests/PresenceIOSTests.swift:97) never changes a cached title, image size or font. Its Dynamic Type assertion checks only height, documented as unaffected by Dynamic Type ([LLP 1059:52](llp/1059-tab-bar-projection.rfc.md:52)). A constant source key retaining the customization bypass can pass; unconditional measurement also passes. **Fix:** assert measurement counts and full-size equality after actual title/image/font changes. Cover state-specific appearance, iPad trait transitions, tvOS and replacement/reset.

The round-1 normal-background installation/removal and legibility fixes hold. The appearance bypass checks only normal/default combinations; other combinations remain unverified. Earlier idle-tick projection ordering and geometry-replay dispositions remain intact. Late raster dimensions are sampled after content updates; cache clearing and queued-report generation guards remain intact. The tab-bar branch is unchanged.

No MATERIAL defect established. Changed source files are 413 and 377 lines; no additional rules or code-quality issue found. `git diff --check` passed. UIKit tests were not run in this read-only environment.