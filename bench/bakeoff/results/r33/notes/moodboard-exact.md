# Moodboard build notes

Completed and installed as `dev.bakeoff.moodboard.b7b2e72` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The finished app is open on Browse with 600 tiles and the data's 62 initial likes.

## Implementation

- Ships the original `data/tiles.json` through the TypeScript data bundle; no network is required.
- Draws HSL-colored square tiles with centered symbols in a virtualized, three-column grid, newest first.
- Native Browse/Liked tabs, ten-board menu plus All, detail metadata, horizontal swipes, and accessible Previous/Next controls.
- Multi-selection supports individual toggles, Select All/Deselect All, batch likes, and confirmed permanent deletion.
- Persistent identifier sets retain likes, unlikes, and deletions. Board filters and each tab's neighbor snapshot also persist.
- Includes a custom mosaic app icon, dark mode, VoiceOver labels, selection haptics, empty states, and layouts checked at the largest Dynamic Type size.

## What was hard

Exact's virtualized collection accepts one direct loop with one element root. Including a scrolling count/filter summary required a first summary row inside that loop. An action named `filter` also collided with the built-in `.filter()` method and had to be renamed.

Emoji need room for their full glyph metrics; line-height 1 clipped them. Automatic verification caught this, along with bottom action buttons that needed to wrap at the largest text size. Selection now uses a compact native navigation title, while the normal title has a natural soft-hyphen break for very large web text.

The first compiler vocabulary command needed permission to write the toolkit's external Cargo cache. Native builds were otherwise straightforward.

## Lookups and decisions

Read Exact's required start guide and looked up tabs, native menus, grid sizing, pan gestures, persistence, assets, tests, and simulator driving in its local documentation and vocabulary commands.

Assumed neighboring tiles should follow the active board and tab's filtered order. Opening a detail captures that sequence; unliking the current tile does not unexpectedly jump away from it. Each tab preserves its own sequence, and deleted tiles are excluded. Equal-date tiles retain the JSON's original order. Dates are formatted in UTC to prevent calendar-day shifts.

## Validation

- `bun exact.mjs verify`: clean; seven tests passed, light/dark checks clean, reload checks consistent, and nothing spilled at the largest text size.
- `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: clean; seven tests passed, light/dark checks clean, and nine screens fit accessibility-extra-extra-extra-large text.
- iPhone scrolling matched UIKit: 0 ms/s measured hitches and a 16.7 ms longest frame during 28.3 seconds of real touches.
- Real simulator touches additionally exercised both swipe directions, native board menus, and multi-selection.
- Final `bun exact.mjs ios --run` installed and launched the verified app. Test changes were cleared from the default store; the finished collection was checked at 600 tiles and 62 likes.
- Screenshots and the unchanged property verification receipt are under `.exact/`.

## Unfinished work

No known unfinished specification features. The property oracle cannot itself deliver Back, drag, or reload events; the web and iOS integration tests exercised those successfully.

## What would make this faster

A virtualized-grid starter with a scrolling summary row, an explicit method-name collision note, and an example of batch actions that wrap under Dynamic Type would have shortened implementation. Exact's automatic layout checks and fast iOS plan updates were particularly useful.
