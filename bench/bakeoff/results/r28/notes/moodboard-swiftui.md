# Moodboard build notes

Finished and installed as **Moodboard**, bundle ID `dev.bakeoff.moodboard.b0ed524`, on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final app is launched. No other simulator was used.

## What shipped

- All 600 original records are bundled, unchanged, in `Resources/tiles.json`.
- Three-column lazy grids, all ten board filters plus All, newest-first sorting, and a separate Liked tab.
- Large square HSL artwork with centered emoji, title, board, and localized calendar date. Native horizontal page swipes plus accessible previous/next controls.
- Individual likes, multi-selection, select/deselect all, batch like/unlike, and confirmed permanent deletion.
- Atomic on-device persistence for likes and deletions; failed writes retain the previous state and show an error.
- Native navigation and menus, light/dark appearances, Dynamic Type, accessibility labels and selected states, empty states, and a custom app icon.

## Decisions and assumptions

The data has date-only values, with multiple tiles on the same day. Equal dates preserve source order. Dates are interpreted in UTC so a western time zone cannot shift them to the preceding day.

Detail browsing uses a snapshot of the currently filtered grid. Unliking a tile while browsing Liked keeps that tile on screen and retains its neighbors for the current browsing session; the underlying Liked grid updates immediately.

Mixed selections offer Like; entirely liked selections offer Unlike. Changing boards clears the selection. Deletion requires confirmation and removes the tile from both tabs permanently.

At accessibility text sizes the decorative introduction is omitted, the board filter can stack above its count, and bulk actions stack vertically. The three-column artwork grid remains three columns as specified. Detail text scrolls independently of its pinned Like control.

## What was hard / what I looked up

The skeleton compiles Swift directly without an Xcode project. Compiler sandbox access required an approved execution context. A complex grid expression exceeded Swift's type-checking budget and was split into a dedicated cell view. The system Bash also needed a nonempty compiler-flags array with `set -u`.

Large-text screenshots exposed crowding and low button contrast in dark mode. Those were corrected and rechecked. I consulted the local README, simulator command help, and compiler diagnostics; no external research or downloaded dependencies were needed.

## Validation

`Tests/StoreChecks.swift` passed checks for all 600 records, unique IDs, all ten board filters, 62 seed likes, newest-first order and stable date ties, individual/bulk liking and unliking, deletion, reloading persistent state, deleting everything, invalid IDs, calendar dates, and failed-save rollback.

On the assigned simulator, separate app processes verified that likes and deletions persisted. Test-only launch scenarios exercised detail pages 1, 2, and 600; unlike during a Liked browsing session; both empty states; board filtering; selection; and the native deletion confirmation. Screenshots were inspected in light mode and in dark mode at the largest accessibility text size. Evidence is in `build/*.png`.

Simulator scenarios can be rerun with:

```sh
MOODBOARD_TESTING=1 SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
./Tests/simulator_checks.sh
```

They use separate test save files. Restore the normal app afterward with the standard build command. The final installed binary was rebuilt without `MOODBOARD_TESTING`; bundle identity, original data contents, and exclusion of test launch arguments were checked.

## Limitations and what would make this faster

No requested product feature remains unfinished. Simulator UI checks used in-app test scenarios and screenshot inspection; physical finger swipes and a full VoiceOver session were not independently driven by an external UI automation tool. Horizontal swiping uses SwiftUI's native page control.

A supplied simulator touch/accessibility automation driver and a ready-made UI test target would make interaction testing faster and more comprehensive. A preconfigured compiler sandbox would also remove the initial build setup friction.
