# Recipes (Exact): notes

## What was built
- **Recipes tab**: all 160 recipes from `data/recipes.json`, which `app.ts` imports so it ships in the bundle. Each row shows the title, category, time ("1 hr 5 min") and a star rating. The nav bar has a Category pull-down (All plus the nine categories, with a checkmark on the current one), a Sort pull-down (Name, Time, Rating, also checkmarked) and the system search field. Search matches the title or any ingredient name, and it combines with the category filter. A section header sums up the current view, e.g. "Soups · 2 recipes · by Time". When nothing matches there is an empty state with a "Show All Recipes" button. The sort choice persists; ties are broken by name.
- **Recipe screen**: category, time, rating, and a Servings row with a native UIStepper (1–99) that scales every ingredient quantity. Scaled amounts show as cooks write them (1½, ⅓, ¾). A footer says "Scaled from N servings." Steps are numbered. A heart in the nav bar adds the recipe to Favorites or removes it.
- **Favorites tab**: the saved recipes sorted by name, each pushing its own detail screen. When it is empty it explains how to add one and offers a "Browse Recipes" button. Favorites are `state … persist`, so they survive a relaunch; a test covers this with `reload`.
- `bun exact.mjs verify` and `verify ios` are both clean: 5 tests, light and dark, the largest text size, and scrolling "like UIKit". The app is installed on simulator C2188CC6….

## What was hard / looked up
- I started from the `tabs` pattern, which already had nearly the right structure. I looked up `menu`, `stepper`, `search`, `choice`, `number-field` and `EmptyState` with `contract vocab`.
- Compiler surprises, each fixed in one round because the messages were clear:
  - An action named `filter` shadowed `.filter()`.
  - `state picks = []` needed a type (`state picks: string[] = []`).
  - A section `header`/`footer` can't sit inside an `if`.
- `EmptyState`'s button has to be indented *under* the part. Written after it, the button landed at the foot of the screen, and I only saw that in a simulator screenshot.
- Guesses:
  - Every ingredient begins with an integer quantity, which I parse as `qty` + the rest. I kept the data's wording ("1 cloves oil") as it is.
  - I round scaled amounts to the nearest ¼/⅓, and an amount under ⅛ shows as "⅛".
  - Each recipe remembers its chosen servings for the session only; this is not persisted.

## Not done
- Favorites has no search of its own. The spec doesn't ask for one.
- Units aren't pluralized or converted when scaling ("3 cup").

## What would have made it faster
- A pattern that already had a filter/sort menu in the nav bar.
- Saying in the `EmptyState` card line that its action button must be its child.
