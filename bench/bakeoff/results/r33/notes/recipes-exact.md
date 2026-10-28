# Recipes (Exact): notes

## What it is
- **Recipes tab:** a large title, the bar's search field (it matches titles and ingredient names), a Category picker (All plus the nine categories), a Name / Time / Rating segmented sort, and a count header. Rows show the title, category, time ("1 hr 5 min") and a star rating. If nothing matches, an empty state offers "Search All Categories" (drops the category, keeps the query) or "Clear Search".
- **Recipe screen:** a stat card for time, rating and category, then a Servings stepper (1–99; a number field with UIKit's stepper). Ingredients are scaled to the servings and shown in kitchen fractions (1½, ⅔, ¼; "cup"/"cups" and "clove"/"cloves" agree with the amount). Steps are numbered. A heart in the bar adds or removes the recipe from Favorites.
- **Favorites tab:** the saved recipes, newest first. Swipe to remove. When empty it shows an empty state with a "Browse Recipes" button. Favorites are a `persist` state (UserDefaults), so they survive a relaunch; the sort choice persists the same way.
- `data/recipes.json` is imported by `app.ts` and bundled into the app. Each ingredient line is parsed once into qty / unit / name.
- `verify` (web) and `verify ios` are both clean: 7 tests, light and dark, the largest text size, and scrolling "like UIKit". The app is built and installed on simulator C2188CC6…, with a generated app icon (`assets/icon-1024.png`).

## What was hard
- **Filter and sort menus in the nav bar.** My first design put category and sort in bar-button pull-down menus. They worked on the simulator with real touches (`--touch platform`). But under `verify ios` the tests' taps on menu rows landed on the agent-painted menu, which sits beneath the navigation bar, so 3 tests failed. I moved both controls into the list's first section as a `select` and a segmented `radiogroup`. That made the tests pass and also keeps the current choice visible. I only found out by running `verify ios`; nothing on the web side warned me.
- **List-row layout.** In a list row, a two-text row became title + right-aligned value: the step text was right-aligned and took half the width. A stat value placed under a caption came out grey. I fixed these by nesting a plain `row`, and by setting `color="-exact-label"` explicitly.
- **Largest text size.** The servings row needed `flex-wrap` and a `min-width` on the stepper field. The segmented labels needed a capped `font-size` (the verifier's own suggestion).

## Looked up or guessed
- I looked up `contract vocab` for stepper, choice, menu, list-rows, swipe, search, persist, EmptyState, section and props, and how to set larger text in a drive (`prefer root-font-size 40`, found by grepping the docs).
- Guesses:
  - Quantities round to the nearest eighth or third.
  - Units are only pluralized for cup and clove. The data itself is odd ("2 cloves eggs"), so I shipped it as is.
  - Search matches ingredient names, not quantities.
  - Category is not persisted; sort is.
- `verify` prints "NOTE … with the oracle's data … does not hold" and "OUTSIDE … unsupported: roster entry toLowerCase". These come from the formal checker, which doesn't have the bundled JSON or `toLowerCase`. They aren't failures, and I left them.

## Not finished / known limits
- On the agent's large-text override, the native number field's own digit doesn't scale. Real Dynamic Type should handle it; I didn't check on a device.
- There's no iPad layout; `deviceFamily` is iPhone only.

## What would have made it faster
- A warning, at compile time or in `test web`, that a test tapping a header pull-down's rows can't pass on iOS.
- A documented `prefer` key for Dynamic Type in drives (`root-font-size` turns up only in a footnote).
- A note in the list-row docs that a row of two texts becomes title + trailing value.
