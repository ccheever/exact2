# Podcasts build notes

Completed and installed as `dev.bakeoff.podcasts.b2b184b` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left running on the normal Episodes screen, in light mode with the standard text size.

## What ships

- All 420 original episodes, copied unchanged into `Resources/episodes.json` and verified against the built bundle.
- Newest-first browsing, unplayed filtering, episode/show search, 12 shows with live episode and unplayed counts, and individual episode summaries.
- Play/pause/resume simulation that marks the episode played; a persistent on-screen player while the app is running.
- Played/unplayed and queue actions in row menus, context menus, and swipes.
- Up Next with native edit-mode removal and drag reordering.
- Atomic local persistence of played overrides and queue order, including duplicate/stale queue-entry cleanup.
- Original geometric show artwork, a custom app icon, dark-mode colors, and layouts that adapt to the largest accessibility text size.

## Validation

Built and launched repeatedly using the provided `build.sh`, exclusively on the assigned simulator. The in-simulator verification suite passed 22 library checks, a persistence setup check, and three assertions after process termination and relaunch. It checks bundled data, ordering, counts, played changes, playback transitions, queue duplicates, reorder/remove operations, disk restoration, and damaged-state recovery.

Reproduce with `python3 Tests/run_checks.py` after building. Reports are in `build/verification/`. Screenshot fixtures and scripts are in `build/screenshots/` and `Tests/`. Reviewed the main screens, empty queue, unplayed filter, playback, queue editing, dark mode, and maximum Dynamic Type size. Fixture launches use separate storage and do not change the normal library.

The provided simctl tooling does not inject touch gestures, so validation used in-simulator model checks and directly launched screen fixtures rather than an automated tap/swipe/drag suite.

## Hard parts and lookups

Date-only publishing values initially shifted a day when UTC midnight was displayed in Pacific time. Display formatting now explicitly preserves the supplied calendar date. Queue edit controls also narrowed rows considerably; edit mode now removes decorative artwork, navigation affordances, and redundant queue badges.

I inspected the local SwiftUI SDK interface for tab accessories. The overload that hides an empty accessory requires iOS 26.1, while this template targets 26.0; the final player uses a compatible safe-area inset. No web research or external dependencies were needed.

## Decisions and remaining work

No requested app features remain unfinished. Playback intentionally produces no audio. Playing an episode leaves it in Up Next until explicitly removed. Same-date episodes use ID order as a deterministic tie-breaker. The data has no artwork or categories, so those presentation details were designed locally.

A supplied simulator touch/accessibility driver, a documented runtime version, and a ready-made screenshot workflow would have made validation faster.
