# Photos (UIKit) — notes

## What's built
- **Library tab**: 4-across square centre-cropped grid (1.5 pt gaps), oldest first, opens scrolled to the newest at the bottom; local-calendar day headings ("Today", "Yesterday", weekday within the last week, then "Sat, Oct 4"); "1,500 Photos" footer; white heart on favorites.
- **Viewer**: modal `UIPageViewController` of zoomable pages at each photo's own aspect ratio. Swipe left and right to page; pinch or double-tap to zoom; tap to hide the bars (the background goes black). It opens and closes with the iOS 18+ `.zoom` transition from the tapped thumbnail, which also gives the interactive swipe-down-to-close (turned off while zoomed in). The toolbar has favorite and delete (with a confirmation).
- **Albums tab**: 2-column grid of the 8 albums (1 column at accessibility text sizes), with the newest photo as the cover and a live count. Tapping an album opens the same grid scoped to that album, without day headings.
- **Select mode**: Select/Cancel, Select All/Deselect All, and the title "N Photos Selected". The tab bar is swapped for a toolbar with favorite (it unfavorites when everything selected is already a favorite) and delete (with a confirmation). Swipe across thumbnails to select a range, as in Photos. Two-finger pan multi-select also works.
- **Persistence**: favorites and deleted ids go in `Application Support/library-state.json`. On first launch, favorites are seeded from `photos.json`.
- Thumbnails are decoded off the main thread with ImageIO, downsampled so the *short* side covers the cell, and cached. Only 240 unique JPEGs back the 1,500 entries, so hits are frequent.

## What was hard or guessed
- There is no UI automation (no idb or cliclick), so I could not tap or swipe in the simulator. I added `DebugHooks.swift`: launch arguments (`-debugAction viewer|select|albums|album|mutate`) that drive screens for `simctl` screenshots. They do nothing in normal launches. As a result, **swipe-down dismissal, paging and swipe-to-select were never exercised by real gestures**. They rely on system behaviour (the zoom transition, `UIPageViewController`) and on code review.
- Found through screenshots: when an album was pushed non-animated, it was laid out after `viewDidAppear`, so the "open at bottom" scroll was skipped. Fixed by keeping the initial scroll pending until it has actually run.
- Guesses: day headings use the device's local time zone (timestamps are UTC). Albums are sorted alphabetically. Deleting goes straight to deletion (no Recently Deleted). Select mode exits after a batch favorite or delete.

## Not done
- No Recently Deleted or undo, no pinch to change the column count, and no thumbnail scrubber in the viewer.

## What would have made this faster
- A simulator tap/swipe tool (or XCTest UI testing) to check gestures directly instead of through launch-argument hooks.
