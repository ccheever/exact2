# Feed — build notes

Finished and installed as **Feed** (`dev.bakeoff.feed.bd60f84`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final installation has the original 400 posts, without test-created content. No other simulator was used.

Implemented the chronological feed, full post text, original-aspect-ratio images, horizontal titled carousels, persistent likes, post detail, full-screen images with downward-swipe dismissal, three local posts per refresh, and a keyboard-focused composer. Detail images have narrower margins than feed images. The viewer also supports pinch and double-tap zoom. Bundled the supplied posts, all 120 JPEGs, and image metadata under `Resources/data/`.

## What was hard

- The input is not chronologically ordered. Posts are sorted using their relative ages, with stable IDs breaking timestamp ties.
- A persistence test caught refresh timestamps overlapping a just-composed post. Refresh batches now share a timestamp with deterministically ordered IDs, preserving their order after relaunch.
- At the largest accessibility text size, the keyboard crowded the draft discard action sheet. The composer now hides the keyboard and uses an alert with explicit discard/keep-writing actions.
- Large text needed stacked metadata in the feed header and wrapping author details. Dark mode needed lighter accent colors. Images retain their ratios at every text size; text is never line-limited.
- The starter has no Xcode project. A small separate UI-test project was created to automate real simulator taps, scrolling, typing, swipes, and relaunches while retaining the provided build script for the app.

## Decisions and references

Read the starter README and used local `simctl` help for screenshots, appearance, and content-size controls. No web research or additional packages were needed. The source provides relative ages rather than calendar dates, so dates are anchored to the feed's initial load and retained in saved state. Composed posts use “You” / `@you`; refreshed posts come from a small rotating set of local examples. Likes and posts are written atomically to the app's Application Support directory.

## Verification

- Built, installed, and launched using `SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run`.
- `Tests/ModelTests.swift` passed: all 400 seed posts, chronological ordering, unique IDs, every referenced bundled image, like/unlike counts, repeated toggles, whitespace-only rejection, long Unicode posts, refresh insertion, and exact state restoration.
- `testCompleteFlow` passed on the simulator: liking, relaunch persistence, post navigation, liking in detail, opening images, downward-swipe dismissal, carousel scrolling and opening, empty/whitespace composer validation, publishing, persistence, pull-to-refresh adding exactly three posts, and persistence of the refreshed feed.
- `testLargeTextAndDarkMode` passed in dark mode at `accessibility-extra-extra-extra-large`: compose, keep-writing/discard handling, and feed scrolling. Screenshots were also visually inspected.
- Test logs and screenshots are in `build/`; UI-test results are in `build/TestDerived/Logs/Test/`. Model tests can be run with `xcrun swiftc -parse-as-library Sources/FeedStore.swift Tests/ModelTests.swift -o build/ModelTests && build/ModelTests`. The complete-flow UI test expects a fresh installation and creates local test posts.
- Restored standard text size and light appearance, removed the UI-test runner, and left Feed installed and running.

## Remaining work and improvements to the toolkit

No required features remain unfinished. A starter UI-test target and a simple simulator gesture-driving helper would have made implementation and verification faster. An explicit reference date for the supplied relative timestamps would also remove the need to choose an initial time anchor.
