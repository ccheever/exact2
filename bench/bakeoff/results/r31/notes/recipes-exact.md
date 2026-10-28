# Recipes (Exact): notes

Built from Exact's `tabs` pattern. It is installed on simulator C2188CC6-… as `dev.bakeoff.recipes.b5d0372` ("Recipes").
`bun exact.mjs verify` passes (5 tests, apart from the web finding below). `verify ios` is fully clean: tests, light and dark, the largest text size and real-touch scrolling.

## What's there
- **Recipes tab:** all 160 recipes from `data/recipes.json`. `app.ts` imports the file, so it is bundled into the app. Each row shows the title, then category · time · ★ rating. A pull-down filter menu offers All Categories and the nine categories, with checkmarks. A pull-down sort menu offers Name (A to Z), Time (quickest first) and Rating (highest first); ties within a sort fall back to name. The bar's search field matches titles and ingredient names. Search, filter and sort all apply together. The section header shows the active category and a count. When nothing matches, an empty state appears with a "Show All Recipes" button.
- **Recipe screen:** shows time, rating and category, then servings with UIKit's stepper (1–99; you can also type a number), a "Recipe makes N" hint, and a Reset button. Ingredients are scaled to the chosen servings and shown as fractions (¼ ⅓ ½ ⅔ ¾) where they land exactly, otherwise to one decimal. Steps are numbered. A heart in the bar and a button at the bottom add the recipe to Favorites or remove it.
- **Favorites tab:** saved recipes, sorted by name, with its own search. Swipe a row to remove it. When empty, it shows "No Favorites Yet", explains how to save a recipe, and has a "Browse Recipes" button.
- **Persistence:** favorites are a `persist` list of ids, so they survive a relaunch (tested with `reload`). The sort order, category and search text also persist, because verify's reload check requires a relaunch to restore what was showing.

## What was hard, guessed or looked up
- I made a few false starts, all fixed:
  - I named an action `sortBy`, which shadowed the built-in `sortBy`.
  - `state x = []` needed a type annotation (`string[]`).
  - `font-weight` and `aria-hidden` must be unquoted values.
- In a list `row`, a second `text` is drawn as a trailing grey "value", like a UIKit value cell. My ingredient and step rows came out right-aligned and grey. I fixed it by nesting an inner `row` (steps) or using one text (ingredients).
- My first empty state put its button outside `EmptyState`, which pushed the button to the bottom of the screen. The card says to indent the button under the call; I found that only by running `contract vocab EmptyState`.
- `system-orange` step numbers failed the contrast check on white, so they are now secondary-label and bold.
- I guessed at how to parse quantities: every ingredient in the data starts with a whole number. A line without a number would be shown unscaled.
- I keep the per-recipe servings as `"id=n"` strings in a non-persisted state list, because `state` can't hold records. A changed servings count lasts for the session; it is not saved across launches.

## Not finished / known
- Web `verify` still reports overlaps at the largest text size, on one screen mid-test: the sort menu open over the list. On the web the agent draws the menu as an in-page overlay; on iOS it is a native UIMenu, and `verify ios` reports nothing there. I left it.
- The ingredient quantity is no longer bold: it is plain text in one line.

## What would have made it faster
- A pattern with a filter or sort menu next to search; that combination is common for list apps.
- A clearer warning that a row's second text becomes a trailing value.
