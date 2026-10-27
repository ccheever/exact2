# Podcasts — build notes

Finished and installed as **Podcasts**, bundle ID `dev.bakeoff.podcasts.b1979ee`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final installed build excludes the DEBUG-only test routes. No other simulator was used.

## Implementation

- Bundles the complete, unchanged `data/episodes.json` as `Resources/episodes.json`: 420 episodes, 12 shows, initially 295 unplayed.
- Newest-first feed with unplayed filtering, title/show/summary search, episode detail, played indicators, list menus and swipe actions.
- Shows list with live episode/unplayed counts and filtered show collections.
- Up Next supports adding, removing, native drag reordering in Edit mode, and menu actions to move earlier/later. Duplicate queue entries are prevented.
- Play marks an episode played and shows a playing/paused state and mini player. There is no audio, as specified.
- Played overrides and exact queue order are saved atomically in the app’s Documents directory. Tests and screenshot fixtures use separate files.
- Original SwiftUI show covers, custom app icon, light/dark colors, native navigation/search/keyboard, and adaptive accessibility layouts.

## What was hard

Date-only values must remain calendar dates: formatting a midnight UTC value in the local timezone initially shifted dates back a day. Parsing and display now both use UTC, and older-year list dates include the year. Full dates are also exposed to accessibility.

At the largest accessibility size, horizontal artwork/menu arrangements left too little room for long words. Artwork and decorative headers are reduced or hidden, episode menus move below the content, metadata stacks, and the filter becomes a native switch. Buttons and summaries remain scrollable and readable. Dark-mode filled buttons use a dark foreground for contrast.

Queue moves need to adjust the destination after removing source indexes; both single and multiple moves are covered. Saved IDs are validated and deduplicated when loading.

The compiler’s macro plugin sandbox could not run within the shell sandbox, so building required elevated tool execution. Simulator commands also needed that execution context.

## What I looked up or decided

Read the skeleton README and inspected the supplied dataset. Used local `simctl` help for appearance and content-size testing; no web research was needed. There were no supplied covers or branding assets, so I designed the artwork and warm orange/ivory visual style. Playing does not automatically remove a queue entry; removal stays explicit. Current playback is session-only, while played state and Up Next persist as requested.

## Verification

All 23 isolated library checks pass, including decoding, sorting, seeded counts, played/unplayed overrides, duplicate prevention, boundary moves, multiple-item moves, removal, playback state, saved-state reconstruction, invalid stored IDs, and malformed-state fallback.

A separate two-process check writes changes, terminates the app, then verifies that played overrides and the reordered queue are restored. Both phases pass. Results are in `build/checks.txt`.

Captured and visually checked simulator screens for the feed, shows, show episodes, detail, playing state, mini player, unplayed filter, search with keyboard, empty search, empty queue, populated queue, Edit controls, dark mode, and the largest accessibility text size. Screenshots are in `build/screenshots/`; `build/installed.png` shows the final non-test build.

Testing limitation: `simctl` has no tap/drag primitive, and the available computer-use app inventory did not expose Simulator. Screenshots therefore use DEBUG-only launch routes, and functional checks exercise the same library methods used by the actual buttons. Native swipe/menu/drag gestures were not directly automated.

To rerun the functional checks:

```sh
PODCASTS_TESTING=1 SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
python3 Tests/run-checks.py
```

Restore the finished build afterward:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

## Unfinished work and faster tooling

No requested feature is knowingly left unfinished. A simulator tap/swipe/type driver would have made direct gesture verification much faster. A ready-made icon asset pipeline and compiler macro sandbox configuration would also have saved setup time.
