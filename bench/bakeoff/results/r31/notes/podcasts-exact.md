# Build notes

Finished Podcasts with bundle ID `dev.bakeoff.podcasts.be04e43`, built and installed on simulator `6266B0E8-9AEB-48C2-93F9-4DA6778D3598` using `bun exact.mjs ios`.

## Delivered

- All 420 episodes from the unchanged `data/episodes.json` ship through a bundled JSON import. Episodes sort by date descending, then numeric ID for stable ties.
- Library search and an Unplayed Only switch; every episode row includes show, title, date, duration, played status, and direct played/queue controls.
- Twelve shows with live episode and unplayed counts, individual show lists, and episode detail screens with complete summaries and simulated Play/Pause state.
- Up Next with duplicate prevention, direct removal, native swipe removal, and Edit controls to move episodes up or down. Boundary moves are disabled.
- Played overrides, queue IDs/order, and the unplayed filter persist through Exact's native persistent state. At this dataset's maximum size, each persisted list stays well under the 16 KB limit.
- Native navigation stacks, tabs, search, grouped lists, switches, semantic colors, dark mode, dynamic text, accessible control labels, and a custom app icon.

## Hard parts and lookups

- Read Exact's start guide and started from the tabs pattern. Looked up navigation, persistence, native lists/swipe actions, switches, theme roles, adaptive text, tests, bundling, and simulator driving.
- Exact's restricted expression syntax required optional `.at()` array access instead of subscripting for queue moves.
- The first large-text checks caught a segmented filter and long titles exceeding their available width. Replaced the filter with a native switch, used semantic text styles, and allowed long title words to wrap.
- A symbol without explicit bounds expanded into large artwork on iOS. Sized it in relative units so the compact artwork still follows accessibility text sizing.
- Shared toolkit caches required sandbox escalation. One verification run became stale when the manifest/icon changed during the run; rerunning after all edits resolved that.

## Decisions

- No audio is produced, as specified. Play marks the episode played and shows Playing; Pause ends that simulated state. Playing itself is transient.
- Reordering uses explicit Move Up/Move Down buttons in Edit mode, which are discoverable and accessible without requiring a drag gesture.
- Dates use an explicit UTC formatter to prevent date-only values shifting to the preceding day in local time zones.

## Verification

- `bun exact.mjs verify`: clean; 9 tests passed; light/dark and largest-text layouts clean.
- `EXACT_SIM=6266B0E8-9AEB-48C2-93F9-4DA6778D3598 bun exact.mjs verify ios`: clean; 9 tests passed; light/dark, largest accessibility text size, persisted state after relaunch, and native scrolling checked.
- Native scrolling physics matched UIKit. The verifier reported zero measured hitches but declined to judge timing because the host machine was busy.
- Final `bun exact.mjs ios` completed and installed the finished app on the assigned simulator.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A podcast/library starter with queue ordering, clearer symbol intrinsic-size guidance, a sandbox-aware shared-cache setup hint, and automatic prevention of stale verification runs during manifest edits would reduce iteration time.
