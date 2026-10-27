# Recipes — build notes

Built with Exact from the `tabs` pattern. `app.contract` is the UI, `app.ts` imports
`data/recipes.json` (bundled into the app) and parses each ingredient into quantity / unit / name.
`app.test.contract` has 4 tests covering every spec item. `bun exact.mjs verify` and `verify ios` both
pass (tests, light and dark, largest text size, relaunch, scrolling). The app is installed on simulator
C2188CC6-… as `Recipes` (`dev.bakeoff.recipes.bef77bd`).

## What's in it
- **Recipes tab**: a large-title list with title, "Category · time" and a ★ rating on each row. The
  navigation bar's search field matches titles and ingredient names. A filter button opens a native
  menu (All Categories + the nine), and a sort button opens a menu (Name A–Z, Time quickest first,
  Rating highest first, with ties broken by name). The subtitle under the title shows the current state,
  e.g. "Drinks · 23 recipes by rating". Category and search combine. When nothing matches you get an
  empty state, with "Show All Categories" if a category filter is on. The sort choice persists.
- **Recipe screen**: category, time and rating, then a Servings row with UIKit's stepper (1–99; you
  can also type a number). Ingredient amounts scale live and show as kitchen fractions (½, ⅓, ¼, ⅔, ¾, ⅛),
  with a "Reset to N Servings" row and a footer saying what the amounts were scaled from. Steps are
  numbered. The heart in the bar adds or removes the recipe from Favorites, and so does a button at the bottom.
- **Favorites tab**: saved recipes sorted by name, with their own search. Swipe a row to remove it.
  When there are none, an empty state explains how to add one and offers a "Browse Recipes" button.
  Favorites are a `persist` state (UserDefaults), so they survive a relaunch. The tests check this with `reload`.

## What was hard / guessed
- **Where controls go.** At first I put the category picker and sort control in the list's first
  section. The compiler pointed out that an `EmptyState` must replace the list, not sit inside it,
  and that would hide the controls on a no-match screen. So both moved into bar-button menus,
  with a header subtitle showing the current filter.
- **Menus.** A `popover` menu placed at the root (outside the route column) drew as floating text
  on iOS instead of a UIMenu. Putting it beside the `header` in the route column, as `apps/agents` does,
  fixed it. Nothing I read said this.
- **Row layout.** Two `text`s in a list `row` become title + right-aligned detail. For the ingredient
  and step rows I wrapped them in an inner `row` to get a plain flex row.
- **The data.** Ingredient strings look like "2 cloves eggs" or "1 cumin", always with a whole-number quantity.
  I guessed a unit list (cup, tsp, tbsp, g, cloves, …) to split unit from name. Search reads names only, so
  "cup" doesn't match every recipe. Fractions round to the nearest common kitchen fraction.
- A sed replace caught more lines than I meant once (`Recipe` inside `shape Recipe[]`, and a
  label in the menu and in the detail rows). A compile error caught the first; a grep caught the second.

## Not finished / known
- At the largest text size, the web build's own sort-menu popover (the web's fallback for a menu)
  runs off the 390-px viewport. That's in web verify only. On iOS it is a native UIMenu and `verify ios`
  is clean. I left it because the target is iPhone.

## What would have made it faster
- An example of a header pull-down menu in the `tabs` / `list-detail` patterns, showing where the
  menu column must live.
- A note in `list-rows` that a row of two texts is title + detail, and how to opt out.
- `agent ios` taps that scroll an off-screen row into view inside a detail list. I had to add a
  `testId` to the list to drag it.
