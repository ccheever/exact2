# Moodboard build notes

Finished and installed as **Moodboard** (`dev.bakeoff.moodboard.b45aa6e`) on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` using `EXACT_SIM=… bun exact.mjs ios`.

## Delivered

- All 600 supplied tiles bundled through the imported `data/tiles.json`, rendered as square HSL colours with centred symbols, in a virtualized three-column grid.
- Newest-first order, All plus ten board filters, separate Boards and Liked tabs, native navigation, and large tile details with title, board and date.
- Real left/right swipe navigation within the opened grid, plus accessible Previous/Next controls and bounded first/last positions.
- Like/unlike, multiple selection, Select All within the filtered board, batch liking and confirmed permanent deletion.
- Atomic local JSON storage for like overrides and deleted IDs. The supplied dataset remains intact. Navigation positions and board choices are also retained.
- A custom app icon, dark mode, accessibility labels, selection haptics, and layouts checked at maximum Dynamic Type.

## Validation

- `bun exact.mjs verify`: clean; six tests passed, light/dark and largest-text checks passed.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: clean; six tests passed, light/dark and largest system text checks passed, including relaunch checks.
- Real simulator touch drives checked both swipe directions, multiple selection, the board menu and the native deletion alert. Maximum-size selection was checked separately in both themes.
- Native scrolling reported “like UIKit,” with no dropped lift frame and no observed hitches in the final run. The machine was heavily loaded, so Exact judged scrolling physics rather than making a final timing judgement.
- Final `bun exact.mjs ios` succeeded and installed the current embedded plan on the specified simulator.

## What was hard

Exact's declarative authoring and typed data seam needed some iteration. A nested scan of all tile IDs exceeded the interpreter's iteration limit; refreshing linear navigation snapshots after bulk changes removed that scan. Native Dynamic Type exposed two layout issues that ordinary-size screenshots missed: oversized selection badges, and a pinned native board picker overlapping scrolled tiles. Bounded badge sizes and a board picker inside the scrolling grid header resolved them. Empty boards keep their picker available.

The first background build also caught a temporary mismatch between the newly configured manifest and the starter data module's old bundle ID. Subsequent builds used the requested ID consistently. Toolkit commands needed access to their shared build cache outside the app workspace.

## Lookups and decisions

Read Exact's required start guide and consulted its tabs, grid/layout, navigation, gestures, file storage, menus, app manifest, testing and iOS-driving references. No external services or downloads were needed.

Dates are date-only values, so they are formatted in UTC to avoid a timezone shifting the displayed day. Tiles with equal added dates retain the source file's order. Each tab has an independent board filter and browsing position. A detail session keeps the neighbouring tiles from the grid it opened; unliking its current tile does not abruptly close it. Deletions are permanent and require confirmation.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A tested virtualized image-grid/detail-pager starter, explicit guidance for bounding decorative symbols at accessibility sizes, and a native picker example inside a scrolling grid header would have reduced iteration. Earlier diagnostics distinguishing pinned-control overlap from text-size allocation would also help.
