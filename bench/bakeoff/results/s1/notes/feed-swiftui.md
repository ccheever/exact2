# Feed build notes

Completed and installed as **Feed**, bundle ID `dev.bakeoff.feed.b9ef42b`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left running on the main feed, with the original 400 posts and standard light appearance/text size. No other simulator was used.

## Implementation

- SwiftUI navigation, lazy feed, native pull-to-refresh, composer sheet with automatic keyboard focus, detail screen, and full-screen photos with swipe-down and close-button dismissal.
- Full post text, original aspect ratios for standalone images, rounded images, horizontal image/title carousels, like toggles and reply counts.
- All supplied JSON and all 120 images are bundled under `data/`, unchanged. An image cache limits repeated decoding.
- Likes, counts, and locally added posts are saved atomically in the app’s Documents directory. Composer drafts survive cancellation and relaunch.
- Light/dark appearance, adaptive Dynamic Type layouts, accessible control labels, haptic feedback, and a custom app icon.

## Decisions and assumptions

The source JSON is not strictly chronological, so posts are sorted by recency rather than source order. Since `minutesAgo` has no absolute timestamp, initial timestamps are anchored to first launch and then persisted. Composed posts use “You” / `@you`. Refresh creates exactly three local text posts; no account or network service is needed. Replies are displayed as counts, as specified.

## What was hard

Keeping nested photo, carousel, like, and navigation interactions independent; retaining original image proportions while giving carousel cards consistent geometry; and adapting the author header to the largest accessibility text size. Screenshot inspection exposed oversized decorative elements squeezing author names, which was corrected by capping avatar size and removing the extra detail icon. The dark-mode accent was also brightened for contrast.

Swift compiler macro plugins and CoreSimulator needed execution outside the shell sandbox. The available desktop-control surface could not access Simulator, so a separate UI-test project was created to drive the installed app directly. It is not part of the shipped app; its runner was removed after testing.

## Verification

- `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run` builds, installs, and launches successfully.
- `python3 Tests/check_data.py` passes: 400 unique posts, 120 images, every referenced image present, and every bundled source file matching its original SHA-256 hash.
- UI interaction test passed: like/unlike counts and persistence across termination, post navigation, full-screen photo swipe dismissal, blank/whitespace rejection, multiline composition and persistence, and refresh adding exactly three persistent posts.
- Carousel/photo/detail/composer UI test passed at standard text size, and again in dark mode at the largest accessibility text size. Detail likes were also checked at that size.
- Exported screenshots were visually reviewed. Results and screenshots are in `build/FeedTests3.xcresult`, `build/FeedVisuals.xcresult`, and `build/FeedAccessibilityFinal.xcresult`, with exported images in `build/visuals/` and `build/accessibility-final/`. Final feed screenshots are `build/feed-final.png` and `build/feed-dark-final.png`.
- Disposable compose/refresh test posts were cleared before final delivery. The final app contains the original feed and preserves future user changes normally.

## Lookups, unfinished work, and improvements to the toolkit

No web research was needed. Compiler diagnostics and `simctl ui` help supplied the needed API/tool details. No specification feature remains unfinished. A preconfigured UI-test target and a ready-to-use simulator gesture/screenshot harness would have made this considerably faster; the supplied skeleton only handled compiling and launching.
