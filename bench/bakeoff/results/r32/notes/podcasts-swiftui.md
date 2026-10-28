# Podcasts build notes

Completed the SwiftUI app and left it built, installed, and launched on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432` with bundle ID `dev.bakeoff.podcasts.be59ec4`.

## What ships

- The complete, unchanged 420-episode JSON is bundled as `Resources/episodes.json`.
- Newest-first episode browsing, search, an unplayed filter, all 12 shows and their live episode/unplayed counts, show-specific lists, and full episode details.
- Played/unplayed actions in row menus, context menus, and swipe actions.
- Simulated play/pause that marks episodes played, plus a now-playing bar.
- A persistent, duplicate-free Up Next queue with native drag reordering, deletion, and accessible move-to-top/bottom actions.
- Original show artwork, an app icon, dark mode, and layouts adapted for accessibility text sizes. At accessibility sizes the filter becomes a full-size switch, decorative headers are omitted, and shows use text-focused cards.

## What was hard

Date-only input must remain a calendar date. Parsing midnight UTC and then formatting in the device timezone initially showed the previous day; explicit UTC calendar-date formatting fixes that.

The starter project has no Xcode project or UI-test target. I added a small standalone XCTest UI-test project under `Tests/` so the installed app could be exercised through real taps, swipes, typing, drag reordering, termination, and relaunch.

Largest-text testing exposed oversized introductory copy and artwork. The accessibility layouts now prioritize episode/show names and controls while allowing content to wrap and scroll.

## Lookups and decisions

Used local command help for simulator appearance/content-size controls and screenshot export from XCTest result bundles. No web research or external dependencies were needed. The simulator's native search dismissal is labeled “Close”; an initial test expected “Cancel,” and the corrected test passed. That failed xcodebuild invocation hung after reporting its failure and was stopped after successful replacement runs.

Episodes with identical release dates are ordered by ID. New queue entries append; adding an existing entry does not duplicate it. Playing an episode marks it played and leaves queue membership under the user's control. Played state and queue order persist; active playback is intentionally temporary because this is a browser with simulated playback and no audio.

## Verification

- `SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 ./build.sh --run` succeeds.
- Model checks in `Tests/LibraryChecks.swift` passed for all 420 records, IDs, ordering, 12-show counts, initial played state, calendar dates, queue deduplication/reordering/removal, playback state, saved-state restoration, stale saved IDs, and load errors.
- Simulator UI tests passed for episode and show navigation, summaries and dates, filtering, list played actions, play/pause, queue changes, persisted state after relaunch, search/no results, swipes, and native drag reordering.
- Final dark-mode tests at `accessibility-extra-extra-extra-large` passed for the full-size filter switch, episode navigation, playback, queue addition/removal, and show navigation. Screenshots were visually reviewed throughout development.
- Successful XCTest results and screenshots are under `build/UITests.xcresult`, `build/SearchQueueTests.xcresult`, and `build/VerifiedAccessibilityTests.xcresult`; earlier layout-review screenshots are in `build/Screenshots/` and `build/FinalAccessibilityScreenshots/`.
- The final installed app is left at standard text size in light mode with the source played states and an empty queue restored after testing.

## Unfinished work

None of the requested features are unfinished. No audio is implemented, as specified.

## What would have made this faster

A starter UI-test target and an example command for interacting with the assigned simulator would have saved setup time. Explicit queue playback/removal semantics and supplied show artwork would have reduced design decisions, though neither blocked implementation.
