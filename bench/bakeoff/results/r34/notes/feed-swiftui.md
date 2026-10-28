# Feed — build notes

Finished and installed as **Feed** (`dev.bakeoff.feed.b6b17f3`) on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The final installation starts with the supplied 400 posts; generated test content and the UI test runner were removed. No other simulator was used.

## Implementation

- SwiftUI feed, post detail, horizontal image/title carousels, and a keyboard-focused text composer with blank-post validation and discard protection.
- Full text without line limits, images sized from the supplied aspect-ratio metadata, and a larger detail image. Full-screen images support swipe-down dismissal, a close button, pinch/double-tap zoom, and bounded panning.
- Like/count changes and all locally created posts are saved atomically in the app's Documents directory. The initial feed timestamps are also persisted.
- Pull to refresh creates exactly three locally authored posts. Composed posts appear first.
- Adaptive light/dark colors, Dynamic Type, accessibility labels, and a custom app icon.
- All source JSON and all 120 JPEGs are shipped in `Resources/data/`, copied byte for byte from `data/`.

## Hard parts and decisions

The input is not chronologically sorted. Posts are sorted by `minutesAgo`, with deterministic ordering for ties. Since the source provides relative minutes rather than dates, dates are anchored at first launch and then saved so they continue aging across launches.

The local posting identity is “You” / `@you`. Refresh cycles through two locally written batches, allocating unique IDs on every refresh. Replies are shown as counts because the supplied data contains no reply bodies.

SwiftUI's toolbar confirmation dialog did not expose a useful keep-writing action in this simulator's presentation. An alert provides clear discard/keep-writing choices instead. Visual testing at the largest accessibility size also revealed that decorative composer copy crowded the editor; that copy is omitted at accessibility sizes, keeping space for multiline editing and the keyboard.

Nested horizontal carousels made a generic scroll-view test query unsuitable for pull to refresh. The main vertical feed has a dedicated accessibility identifier, and the tests use a deliberate pull gesture.

## Verification

The skeleton README, JSON schema, and image metadata were inspected locally; no web lookup or external packages were needed. A small Xcode XCTest harness was added alongside the required `build.sh` workflow.

Passing simulator checks include:

- All 400 records, all referenced image files, image decoding, ordering, and unique IDs.
- Like/unlike count changes, including initially liked posts, with restoration from disk.
- Whitespace rejection, preservation of long multiline text, and exact three-post refresh batches.
- Composing, carousel scrolling, post navigation, fullscreen image opening, close and swipe dismissal.
- Likes, composed posts, and refreshed posts surviving application termination and relaunch.
- Draft discard/keep-writing behavior.
- Dark mode with `accessibility-extra-extra-extra-large` text, navigation, multiline editing with the keyboard, image zoom/panning, and dismissal.

The main suite passed four tests (`build/FinalTests.xcresult`). The final largest-text/dark-mode suite passed two tests (`build/AccessibilityFinal.xcresult`). Exported review screenshots are in `build/AccessibilityFinalScreenshots/`. The final app was built, installed, and launched with:

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```

Nothing in the requested specification remains unfinished. A starter with an existing UI-test target and a documented data schema would have made this faster.
