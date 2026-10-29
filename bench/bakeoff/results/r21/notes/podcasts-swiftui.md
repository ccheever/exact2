# Build notes

Completed Podcasts (`dev.bakeoff.podcasts.d37cd6`) and left the normal build installed and launched on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

## What works

- All 420 episodes are bundled unchanged in `Resources/episodes.json`, newest first, with show, title, full date, duration, and played status.
- All/unplayed filtering, episode/show search, 12 show cards with live counts, and individual show lists.
- Episode summaries, simulated Play/Pause, a compact player, and a player detail sheet.
- Played/unplayed and queue actions in each list row’s menu and swipe actions.
- Up Next deduplication, native Edit/reorder/delete, and accessible Move Earlier/Move Later menu actions.
- Played state and queue order persist locally. Test state is isolated from normal app state.
- Light/dark themes, native navigation/search/keyboard, and accessibility layouts at the largest text size. The compact player uses a bounded text size because the system accessory has a fixed height; its accessibility label exposes the full title.

## What was hard

Date-only JSON values must stay date-only: parsing at UTC midnight initially shifted dates backward in the device timezone. Parsing with a Gregorian calendar in the device timezone fixed this. iOS 26’s floating tab accessory also needed explicit conditional visibility to avoid an empty player bar. The player sheet retains its episode independently of current playback so pausing never leaves a blank sheet.

Large text required replacing segmented filters with toggles, using a single-column show grid, removing decorative thumbnails from episode rows, and allowing long labels to wrap. Native list rounding also required extra space around custom section content.

## Decisions and assumptions

There were no supplied covers or visual guidelines, so I created a coordinated set of SwiftUI show illustrations, an app icon, and a warm cream/forest/orange palette. The small show category labels are editorial presentation choices. Playback is simulated, as requested. Playing marks an episode played immediately; pausing keeps it played. Playing does not automatically remove an item from Up Next. Queue additions append once, and users explicitly remove items. The transient playing state resets on relaunch; played status and the queue do not.

No external documentation or web research was needed. Compilation identified that the accessory visibility API requires iOS 26.1; there is an availability-checked fallback for iOS 26.0.

## Verification

`Tests/check.sh` builds a test-only configuration, runs 27 XCTest-free checks inside the assigned simulator, verifies persistence from a new app process, then rebuilds, installs, and launches the normal app. Final results are in `Tests/model-checks.log` and `Tests/relaunch-checks.log`; all checks passed. Checks cover data/counts/sorting, local calendar dates, played transitions, queue deduplication/reorder/removal, boundary actions, persistence, and invalid saved data.

Simulator screenshots in `Tests/` were inspected for the episode browser, show grid, show episode list, details, search with keyboard, populated/empty queues, native editing handles, dark mode, and maximum accessibility text size. The bundled JSON was verified byte-for-byte against the supplied file. Test-only entry points are compiled out of the installed normal build.

No requested features remain unfinished. The provided simctl toolkit does not inject touch gestures, so UI verification used test-only screen routing and screenshot inspection rather than automated tap/swipe tests.

## What would make this faster

A simulator touch/accessibility automation interface would make end-to-end gesture checks faster. Providing cover artwork and a design direction would reduce visual design iteration. A starter that includes optional test compilation and an app-icon hook would also help.
