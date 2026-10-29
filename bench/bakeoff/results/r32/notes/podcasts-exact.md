# Podcasts build notes

Finished and installed as **Podcasts** (`dev.bakeoff.podcasts.b5145a3`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## Delivered

- All 420 episodes, newest first, with the supplied initial played states.
- Unplayed filter, search, 12 shows with live episode/unplayed counts, and native navigation to show and episode screens.
- Episode summary, date, duration, Play/Pause state, and played/unplayed actions.
- Episode menus support marking played/unplayed and adding to Up Next directly from a list.
- Persistent Up Next with native drag handles, Earlier/Later controls, and removal. Duplicate additions are ignored.
- Offline catalog, native tabs and controls, dark mode, Dynamic Type, and an app icon.
- The original `data/episodes.json` is unchanged. An exact copy is shipped in `assets/episodes.json`; the offline data module also imports the catalog. The packaged app's catalog was checked byte for byte.

## What was hard and what I looked up

I started with Exact's tabs pattern and consulted its guide and vocabulary for navigation, menus, persistence, virtualized lists, and reorder handles. Feed lists must use the virtualized list configuration rather than the grouped-list configuration. Reordering requires string keys and a virtualized list.

The largest text size exposed crowded filter and row layouts. Native verification also found overlap and dark-mode contrast in the original segmented filter strip. Moving the filter to the navigation bar and allowing rows and titles to wrap resolved those findings without reducing the user's text size.

Exact's temporary agent plans and installed plans have different freshness rules. Adding bundled assets required rebuilding before the final native verification. Web and native relaunch navigation also differ, so persistence assertions use the saved data rather than assuming the same route is restored.

## Decisions

Date-only values are formatted in UTC so time zones cannot shift an episode to the previous day. Episodes with equal dates keep their source order. Shows are alphabetical. Queue additions append; playing or marking an episode does not remove it from the queue. Played changes and queue order persist, while the simulated Now Playing state lasts for the current session. No audio is implemented, as specified.

## Validation

- `bun exact.mjs verify`: clean; 8 tests passed, light/dark layouts clean at both widths, no spill at the largest text size.
- `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: clean; 8 tests passed, light/dark layouts clean, no spill at accessibility-extra-extra-extra-large (53 pt body).
- Native scrolling physics matched UIKit. The machine was heavily loaded, so Exact did not judge timing performance.
- Real simulator touches verified episode menus, filtering, navigation, Play, and dragging the third queue item to the first position.
- Final `bun exact.mjs ios --run` left the verified app installed and launched normally.

No requested features remain unfinished.

## What would have made this faster

A starter combining native tabs, a long virtualized episode list, persistent queue ordering, and accessible row menus would save setup time. Clearer guidance about grouped versus virtualized lists and asset changes invalidating installed builds would also help.
