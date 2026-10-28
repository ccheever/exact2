# Recipes (Exact): notes

## What it is
- `app.contract` is the view: a Recipes tab and a Favorites tab, each with its own navigation stack.
  - **Recipes:** a large title and the bar's search field, which matches titles and ingredient names. Two bar pull-down menus: Category (All plus the nine categories, with checkmarks) and Sort (Name, Time, Rating). The section header shows the count and the active category. "Show All Recipes" clears the filters. An empty result shows an `EmptyState` that explains why and offers to clear.
  - **Recipe screen:** time, servings, rating and category; a servings stepper (UIStepper plus an editable number field, 1–99); ingredients scaled to cook-style fractions (¼ ⅓ ½ ⅔ ¾); "Reset to N servings"; numbered steps; a heart in the bar to add or remove the recipe from Favorites.
  - **Favorites:** the saved recipes. Swipe a row to remove it. When empty, a helpful empty state with a "Browse Recipes" button.
- Favorites (and the chosen sort) are `state … persist`, so they survive relaunching the app (tested with `reload` and verify's relaunch).
- `app.ts` imports `data/recipes.json`, so the data is bundled with the app. It splits each ingredient into a leading amount and the rest, which makes the amount scalable.
- Tests: `app.test.contract` has 5 tests covering every spec item. `verify` (web) and `verify ios` both pass the tests. `verify ios` found nothing in light or dark mode, nothing spills at the largest accessibility text size, and scrolling has nothing for the app to fix.

## What was hard / guessed
- Naming my actions `filter` and `sortBy` shadowed the built-in `.filter` and `sortBy`. The compiler explained it clearly; I renamed them.
- A grouped-list row of two `text`s is read as a title/value cell, which pushed the ingredient name to the right like a value. I wrapped the quantity (and the step number) in a `view` so the row is custom. I found this in contract-for-humans.md, not the card.
- A section `footer` must hold exactly one `text` and can't sit under an `if`. I moved Reset into its own row.
- Guesses:
  - The ingredient data is all "N unit name" with integer N, so scaling only touches the leading number.
  - Amounts round to the nearest ¼ or ⅓ (⅛ for tiny ones).
  - The servings value is kept per recipe for the session only, not persisted.
  - The sort persists; the category filter does not.
  - Favorites are listed by title.
- Category order is mine (Breakfast → Drinks), not alphabetical.

## Not finished / known leftovers
- Two compiler warnings remain:
  - Ingredient and step rows are keyed by position. The data has no ids for them, and they never reorder.
  - Two SF Symbols (`sf/clock`, the filled filter glyph) would be blank on the web build. This is an iPhone app, so I left them.
- I did not hand-check the web build at the largest text size: the web `verify` reported overlap in its header at that size. iOS uses native bars and verify ios reported nothing.

## What would have made it faster
- A pattern that already had a bar filter menu and sort menu over a list.
- Having the card say that a row's "image, text, text" shape becomes a UIKit value cell.
