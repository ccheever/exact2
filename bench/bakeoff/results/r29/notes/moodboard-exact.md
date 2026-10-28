# Moodboard build notes

Completed the requested app, built it with Exact, and left it installed and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86` as `dev.bakeoff.moodboard.bd62e11`.

## What ships

The original 600 records are included verbatim as `assets/tiles.json`; it is identical to `data/tiles.json` and is present inside the built app bundle. The data module imports that asset and converts HSL values to display colours. A custom app icon also ships.

The app has a virtualized three-column grid, native tabs and navigation, a board menu, large tile details with horizontal swipes and accessible previous/next controls, individual likes, multiple selection, bulk likes, and confirmed bulk deletion. Atomic local JSON writes preserve likes and deletions.

## Decisions and assumptions

- Newest dates come first; equal dates retain the supplied file order.
- Detail neighbours follow the current board and All Tiles/Liked context.
- An unliked tile remains visible in its open detail until browsing away, but disappears from the Liked grid.
- Changing boards or tabs clears the selection. Filters and navigation reset on relaunch; likes and deletions persist.
- An empty saved library remains empty rather than being reseeded.

## What was hard and what I looked up

I used Exact's start guide and tabs pattern, then looked up grid tracks, aspect ratios, virtualized lists, pointer events, file grants, atomic writes, native menus, navigation bars, and simulator touch driving.

A quadratic neighbour calculation exceeded Contract's iteration limit with all 600 tiles. Linear filtering fixed it. Exact's test language also requires checking nonempty arrays by individual elements.

Large Dynamic Type exposed problems with controls pinned beside a virtualized grid. Moving filtering and selection actions into the native navigation bar produced a cleaner layout. Standalone rem sizes work, but rem inside min/max sizing expressions is not supported.

Visual simulator review caught a command-only Delete button that did not appear in the native toolbar. An explicit press action calling `showModal` fixed it. Real touches verified that the visible button opens the confirmation alert and that both Cancel and Delete work.

Changing sources while verification runs invalidates its build receipt. Final checks were run with the source held stable. The web verifier has scoped allowances for the intentional overlap of an open board popover; final native verification reported zero findings and used no allowances.

## Validation

- `bun exact.mjs verify`: clean; 9 tests passed, including relaunch persistence and the largest text size.
- `EXACT_SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 bun exact.mjs verify ios`: clean; 9 tests passed, light/dark layouts had no findings, and nothing spilled on 11 screens at the largest accessibility text size.
- Native scrolling physics matched UIKit.
- A separate real-touch test confirmed bulk deletion, swipes in both directions, liking, and tab navigation. A fresh process retained both the deletions and likes in its isolated test store.
- Final standalone installation and launch completed with `bun exact.mjs ios --run` on the required simulator.

Nothing from the requested specification remains unfinished.

## What would have made this faster

A tested native gallery pattern with a virtualized grid and paging gestures, clearer examples for toolbar confirmation buttons, and earlier guidance about large-text layout beside native lists would have reduced iteration. Explicitly warning when a command-only header button will not project into the toolbar would also help.
