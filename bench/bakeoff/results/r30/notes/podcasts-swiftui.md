# Podcasts — build notes

Completed and installed as `dev.bakeoff.podcasts.bc3c631` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final app is running with the original played flags, an empty Up Next queue, light appearance, and standard text size. No other simulator was used.

## Delivered

- All 420 bundled episodes, newest first, with show, title, calendar date, duration, and played status.
- All/unplayed filtering, episode and show search, 12 shows with live counts, and show-specific episode lists.
- Episode summaries and simulated play/pause, plus a mini-player on iOS 26.1 and later.
- Played/unplayed controls directly in rows and swipe actions and menus for queue management.
- Persistent played overrides and an ordered, duplicate-free Up Next queue with native editing, dragging, and removal.
- Original SwiftUI cover artwork, an app icon, dark colors, accessibility labels, and layouts adapted for the largest Dynamic Type size.

`Resources/episodes.json` is an unchanged copy of `data/episodes.json` and is included in the built application.

## Verification

The final standard-size simulator suite passed all three workflows: browsing/filtering and persistence, queue reorder/removal and persistence, and search/detail/playback including the mini-player sheet and switching tabs while playing. A separate dark-mode run at `accessibility-extra-extra-extra-large` passed and produced eight screenshots covering the main screens and editing/playback states. Screenshots were visually inspected.

Model checks passed for the 420 records and 12 shows, ordering, dates, initial and overridden played state, queue deduplication/reordering/removal, persistence, playback transitions, stale IDs, and corrupt saved-state recovery. Test sources are in `Tests/`; final simulator results are `build/FinalUITests.xcresult` and `build/AdaptiveTests3.xcresult`. Standard interaction workflows assume a freshly installed library and run in their browse → queue → search order; the adaptive workflow is run separately. The temporary test runner was removed before the final installation.

## What was hard / what I checked

Date-only JSON values initially shifted back a day when displayed in the local time zone; parsing and display now both preserve the supplied calendar day. The first compiler invocation also needed access beyond the shell sandbox for Swift macro plugins. Native tab accessories needed explicit enable/disable behavior to avoid an empty player capsule, and accessibility layouts needed less decorative content and more width for episode titles.

I consulted the starter README, simulator command help, and installed SwiftUI SDK interfaces. No web lookup was needed. I chose the visual identity and show artwork because none was supplied. Playing is intentionally a session-only visual state; played flags and the queue are the durable state. Playing does not automatically remove an episode from the queue.

## Remaining work / faster next time

All specified features are complete. Audio is intentionally absent, as requested. A ready-made simulator UI-test target and interaction harness in the starter would have made validation faster.

Rebuild and launch with:

```sh
SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run
```
