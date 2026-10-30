# Podcasts — build notes

Finished and installed as `dev.bakeoff.podcasts.b71f56c` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final installation is clean: all 420 bundled episodes, the original 125 played / 295 unplayed states, and an empty Up Next queue. No requested feature is unfinished.

## Implementation

- SwiftUI app with Episodes, Shows, and Up Next tabs, native navigation, search, swipe actions, row action menus, and native queue edit/reorder controls.
- `Resources/episodes.json` is an exact copy of the supplied data and ships in the bundle. Twelve show covers are drawn with SwiftUI; the app icon is bundled locally. No network dependency.
- Played overrides and ordered queue IDs are written atomically to Application Support after every change. Saved IDs are validated and queue duplicates are removed when loading.
- Playback is a simple session state with Play/Pause and a now-playing bar; playing marks the episode played. Audio is intentionally absent, as specified.
- Dark mode uses an adaptive accent. At accessibility text sizes, decorative headers and thumbnails are reduced, row menus move below text, and content wraps in scrollable screens.

## Decisions and difficult parts

- Date-only strings must be parsed in the local time zone; midnight UTC initially displayed the previous day in Pacific time. Fixed and verified against the supplied dates.
- Queue semantics were unspecified, so each episode can appear once, new additions append, and playing an episode retains it in Up Next until manually removed. Playback itself is temporary; played state and queue order persist.
- Equal publication dates use episode ID as a stable secondary sort key.
- The starter has no Xcode project. I added a small UI-test-only project under `Tests/` without changing the app build workflow.
- Compiler plugins, SDK caches, and CoreSimulator required execution beyond the restricted shell sandbox. An early XCTest-managed relaunch stalled; independent `simctl` termination/relaunch plus UI-test activation worked reliably. Later complete test runs passed normally.
- Consulted the local README, build script, simulator UI help, and local test-result tools. No external documentation or services were needed.

## Verification

Passed model checks for all 420 records, newest-first ordering, 12 shows, initial counts, played/unplayed overrides, queue deduplication, reordering, removal, persistence, and invalid saved-ID cleanup.

Passed simulator UI checks for list actions, native queue dragging with order assertion, queue removal, show counts and navigation, episode summary and Play/Pause, unplayed filtering, keyboard search and empty results. A separate test passed after actual `simctl terminate` / `launch`, confirming restored played state and Up Next contents. All primary screens were exercised in dark mode at the largest accessibility text size and screenshots were reviewed. Test logs and screenshots are under `build/`.

The finished app was rebuilt, installed, and launched after testing; the temporary UI test runner was removed. Simulator appearance and text size were restored to light / standard large.

## What would make this faster

A supplied UI test target and simulator automation harness would reduce setup time. Explicit queue behavior and provided show artwork would remove small product/design assumptions. The existing command-line build script made app compilation and installation straightforward.
