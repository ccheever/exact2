# Feed — build notes

Finished, built, installed, and launched as `Feed` (`dev.bakeoff.feed.b2b5208`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was booted or used. The final installed executable is the production app, not the validation harness.

## Implementation

- Ships the original 400-post JSON, image metadata, and all 120 JPEGs under the bundle’s `data/` directory. Compared every bundled file byte-for-byte with its source.
- Sorts seed posts by `minutesAgo`; the input file itself is not chronological. Seed times become stable publication dates at first launch, so time continues advancing across relaunches.
- Native SwiftUI navigation, lazy vertical feed, horizontally scrolling image cards, expanded post detail, and a black full-screen photo viewer with downward swipe dismissal, explicit Close, pinch zoom, double-tap zoom, and panning while zoomed.
- Likes, composed text posts, and locally generated refresh posts save immediately to an atomic JSON file in Application Support. Refresh adds exactly three posts with unique IDs. Compose rejects whitespace-only text and preserves paragraphs and emoji. Cancelling a nonempty draft asks before discarding it.
- Adaptive light/dark colors, Dynamic Type, accessibility labels/actions, reduced-motion handling for custom animations, image caching, and a generated leaf app icon.

## What was hard / decisions made

The direct `swiftc` skeleton has no Xcode project, asset catalog, or UI-test target. I kept the app compatible with the supplied build script and created a separate compile-time validation entry point under `Tests/`. Validation uses its own persistence file and does not add test posts or likes to the normal feed.

Largest accessibility text required careful wrapping of author metadata and carousel titles. A screenshot caught the composer avatar’s scalable letter overflowing its fixed circle; the letter now uses an appropriate fixed decorative size.

I interpreted “image larger” in detail as wider media margins plus larger post text; the full-screen viewer can display the full image and zoom. Since no signed-in profile was supplied, composed posts use “You” / `@you`. Refresh posts use a rotating set of locally written fictional updates. Reply counts are informational, as requested.

I consulted the skeleton README, data schemas, simulator command help, and the available computer-use guidance. No internet lookup or external dependencies were needed. The first icon generation approach relied on AppKit drawing unavailable in the headless environment; the finished icons use Core Graphics and ImageIO directly.

## Verification

On the assigned simulator, the validation build passed checks for all 400 seed posts, chronological ordering, unique IDs, decoding every referenced image, liking/unliking (including initially liked posts), whitespace-only composition, very long text with paragraphs and emoji, repeated refreshes, exact persistence round-trips, and restoration after terminating and relaunching the process. The persisted validation feed contained 407 posts.

Inspected simulator screenshots of the main feed in light mode and dark mode, the feed and post detail at the largest accessibility text size, the composer with its keyboard, carousel titles and action counts at the largest size, and the full-screen image viewer. Final launch screenshot: `build/final-feed.png`. Validation output: `build/validation.log` and `build/relaunch.log`.

Interactive simulator control was not available through the supplied UI connection, and `simctl` provides no touch-event command. Therefore taps, horizontal swipes, pull-to-refresh gestures, and photo pinch/dismiss gestures were not driven end-to-end by automation. Their underlying model operations were exercised, screens were rendered in the simulator using the separate validation entry point, and the gesture/navigation code was reviewed. No known feature is left unimplemented.

The simulator was restored to light appearance and standard (`large`) text size. All test entry points are excluded from the production build.

## What would have made this faster

An authorized simulator touch/accessibility driver, or a supplied UI-test harness, would enable complete gesture-level verification without adding a temporary screen-rendering entry point. A headless app-icon template would also reduce setup time.

## Rebuild

`SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run`

`Tests/build-validation.sh` installs the validation executable using the same bundle ID. After using it, run the production build command again to leave the finished app installed.
