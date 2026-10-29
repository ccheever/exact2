# Build notes

Completed Podcasts (`dev.bakeoff.podcasts.ba61184`) and left the final build installed and running on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was booted or used.

All seven requested features are implemented. The original 420-episode JSON is shipped unchanged in the application bundle. Episodes are sorted by descending calendar date, with ascending ID as a deterministic tie breaker. Shows have live episode and unplayed counts. Lists support swipe actions, menus, and context menus. Up Next supports native editing, dragging, deletion, and accessible Move up/Move down menu actions. Played overrides and queue order are saved locally and restored on launch.

## What was hard

The main work was keeping every screen consistent when played or queue state changes, restoring state without duplicate or stale queue IDs, and making the largest accessibility text size usable. At accessibility sizes the decorative headings and cover thumbnails yield space to content, the segmented filter becomes a labeled switch, search uses a larger native text field, and queue/status labels stack. Dark mode uses adaptive backgrounds, accents, and brighter show labels.

The starter had no Xcode project or testing harness. I added a small project under `Testing/` for automated simulator tests while keeping `build.sh` as the normal build path. Sandbox restrictions required escalated simulator/Xcode tool execution.

## Lookups and decisions

I checked `simctl ui` help for appearance and content-size settings. No internet research or downloaded dependencies were needed. I chose custom cover identities and an app icon because no artwork was supplied. Queues prevent duplicate episodes; playing an episode leaves it in the queue until explicitly removed. The current playing indicator is transient; played state and the queue persist. Dates are treated as calendar dates without timezone shifts.

## Verification

- Verified all 420 unique IDs, all 12 shows, valid dates/durations, and byte-for-byte equality between source and bundled JSON.
- Normal-size regression: four tests passed, zero failures. Covered list marking/filtering, queue order and removal, native edit mode, Play state, relaunch persistence, show counts/navigation, search, and empty results.
- Largest Dynamic Type in dark mode: passed simulator UI checks and inspected screenshots of episodes, details, shows, and queue. Refined and visually rechecked header wrapping in the final installed build.
- Isolated model checks passed for sorting, filtered search, duplicate prevention, moves, batch deletion, played overrides, relaunch, invalid saved IDs, and boundary moves.
- Final build succeeded through the supplied build script. Final screenshot: `build/final-episodes.png`. Test logs, screenshots, and result bundles remain under `build/`; reproducible commands are in `Testing/README.md`.

## Unfinished work and faster setup

No requested feature is unfinished. Audio is intentionally simulated, as specified. A supplied simulator test project and an established artwork/style reference would have made the build faster.
