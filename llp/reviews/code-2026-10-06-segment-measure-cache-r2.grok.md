# Code review: a segmented tablist is measured only when what sizes it changes, round 2 (9dc40108e), 2026-10-06 (grok)

- **Family:** xAI. `grok-4.7` via `~/.grok/bin/grok`, reasoning effort xhigh, `--no-subagents`, `--cwd` a detached worktree at `9dc40108e`, plain output.
- **Method:** one brief (sha256 `04a7497ddb82afbc2afc1eccb3fff967f36967f2492b74bf394ba49fbc72fe43`), shared with astra. Round 2, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the review's findings and verdict, unedited.
- **Verdict:** LAND.

---

LAND

Round 1 holds. Legibility weight is in the key (`SegmentsIOS.swift:208`). A normal-state background or a normal/normal divider is measured every time and stored as nil (`:211-215`). `clearSize` drops that entry before `sizes` (`:197-201`). The tab-bar branch is still `sizeThatFits` (`:235-238`). No `code-2026-10-05-segment-measure-cache*` review exists.

1. **MINOR** — `testASegmentedTablistIsMeasuredAgainWhenWhatSizesItChanges` passes if the key drops titles, fonts, image size, text size, and legibility weight. `PresenceIOSTests.swift:95-127` installs and removes a normal background (that part fails a cache that never invalidates: on iOS 27 a 60pt normal background changes the height from 31 to 60) and then compares only height after `preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge`. That category leaves the size at `(112, 31)`, so the height assertion matches a stale report. The comment names a title change; the body never changes a title, a font, or an image. Fix: `setTitle` a longer string and `setTitleTextAttributes` a larger font, and assert the full reported `CGSize` equals a fresh `intrinsicContentSize`. Assert `reports.count` stays put when the size is unchanged.

2. **MINOR** — The reported width goes stale for title attributes other than font name and point size, for a divider that is not the normal/normal pair, and for `setWidth`. `SegmentsIOS.swift:205` keeps `fontName` and `pointSize` for `.normal` and `.selected`. On iOS 27, kern 12 moves the width from 130 to 250, a shadow from 112 to 130, and a baseline offset from 112 to 130, with the height staying 31. A divider set only for selected/normal widens the control from 112 to 120 while `dividerImage(forLeftSegmentState: .normal, rightSegmentState: .normal, barMetrics: .default)` stays nil (`:212`). `setWidth(80, forSegmentAt: 0)` widens it to 174. Layout uses the height alone (`kernel/src/style.rs:1429`). Fix: put both attribute dictionaries and each `widthForSegment` in the key, and treat a divider for selected/normal or normal/selected as customized.

Checked, no visible difference from before:

- The minimum height on iOS 27 stays 31 through a font of 80pt, accessibility XXXL, bold text, regular/compact size class, a 200×80 segment image, a 70pt divider, content offset, and a selected-only or highlighted background. A normal background is the input that changes it (60pt plain, 44pt resizable), including when a 90pt selected image is also set (height follows the normal image, 40). That path bypasses the cache.
- Late images are drawn into `icon.bounds` before measure (`SegmentsIOS.swift:145-156`, `:370-377`). A new bounds changes `image.size`, which is in the key. Same bounds, same intrinsic size.
- An appearance-proxy background set before the control is in a window leaves both the getter and `intrinsicContentSize` on the default. Compact and prompt metrics do not size a control that is not in a bar (UIKit logs that prompt images are ignored; a compact-only image left the size at 31).
- Size class does not change the size. Idiom does not change at runtime. tvOS uses this same function; focus is not an intrinsic-size input.
- A cache hit still goes through `sizes` (`:241`). Padding and border are applied after the cache (`:230-234`). Switching to a tab bar does not read `naturals` (`project` at `:263`); `removeBar` clears it on the way back (`:191-194`).
- Idle-tick holes stay closed. `segments.sync()` still runs at the end of a full apply (`PresenterIOS.swift:961`) and on the out-of-batch projection sync after grouped lists (`:36`). Sync still updates the frame, enabled, selection, and membership (`SegmentsIOS.swift:361-376`) before measure. Those earlier holes were sync not running, and `prepare()` writing an old carried frame. This skip is `intrinsicContentSize` only.
- `SegmentsIOS.swift` is 413 lines, the test file 377. `git diff --check` is clean. No new rule surface.
