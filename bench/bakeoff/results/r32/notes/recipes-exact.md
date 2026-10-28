# Recipes (Exact) — notes

Built with Exact from the `tabs` pattern. It is installed on simulator C2188CC6-FBE9-409A-92A8-ABD89AC2C432 as
`dev.bakeoff.recipes.b8f771e` ("Recipes").

## What's there
- **Recipes tab:** a large-title list of all 160 recipes. Each row shows the title, then category · time, then a
  star rating. The bar has the search field (it matches title or ingredient name), a **Filter** pull-down
  (All Categories + the nine categories, with a checkmark on the current one) and a **Sort** pull-down (Name /
  Time / Rating, with a checkmark). The filter and search work together. The subtitle under the title shows the
  current filter and count ("Soups · 7 recipes"). When nothing matches, an empty state offers "Show All Recipes".
  The sort order is saved between launches; the category resets each launch.
- **Detail:** the title, category and rating, then Time, then Servings with the native UIStepper (the field
  can also be typed in). Under that are the ingredients scaled to the servings, shown as kitchen fractions
  (1½, ⅔, ¾) with "cup"/"cups" and "clove"/"cloves" matched to the amount. Steps are numbered. A footer says what
  the quantities are scaled from. If the field is left blank or 0, it goes back to the recipe's own servings
  when you tap Done. A heart in the bar adds or removes the favorite.
- **Favorites tab:** saved recipes in the same sort order, with their own search. You can swipe left on a row
  to remove it. When there are none, an empty state explains how to add one and has a "Browse Recipes" button.
- **Persistence:** favorites are a `persist` state (a list of ids), so they survive relaunch. A test covers this.
- **Data:** `app.ts` imports `data/recipes.json`, so the file is bundled. At load it splits each ingredient
  into amount, unit and name, and builds a lowercase search string.
- **Checks:** 6 tests pass on web and iOS. `verify ios`: no findings in light or dark mode, nothing spills at
  the largest accessibility text size, and scrolling is "like UIKit".

## What was hard / guessed
- Action names shadow built-ins: my `filter` and `sortBy` actions broke `.filter()` and `sortBy()`
  everywhere. The compiler said so clearly, so the fix was quick.
- `font-weight` takes numbers (700), not "bold" or "semibold".
- An `EmptyState`'s button has to be *indented under* the part. Written as a sibling, it sat at the bottom of
  the screen. The vocab entry explains this, but I found it after I saw the screenshot.
- iOS's number field clamps to `min` by itself, so a test that typed "0" passed on web and failed on iOS. I
  changed it to test a blank field.
- Scaling: the data's quantities are all whole numbers with odd units ("2 cloves eggs"). I chose to round
  scaled amounts to the nearest ¼ or ⅓ and show them as fraction glyphs, which is how cooks read
  amounts. Amounts below ⅛ show as decimals.
- The category filter is a bar pull-down menu (10 options is too many for a segmented control). I chose that
  over a strip of chips I'd have to draw myself, so it stays native.

## Not finished / known
- Two compiler warnings remain (`lower-key-position`): the ingredient and step lists use their position as
  the key. Those lists never change, so this is harmless.
- `verify` on web reports the sort menu overlapping rows at the largest text size. That happens only in the
  web-painted popover the test passes through; on iOS it is a UIMenu.
- The detail shows the title both in the nav bar (inline) and as a large heading in the first card.

## What would have made it faster
- An example of a filter/sort pull-down in a list pattern (I put the menu, menuitemradio and bar-buttons
  docs together myself).
- A warning when an action name shadows a standard function, given at the point where the action is declared.
