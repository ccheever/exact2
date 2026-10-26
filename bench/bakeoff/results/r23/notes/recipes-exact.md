# Recipes (Exact) — notes

Installed as `dev.bakeoff.recipes.b04207b` ("Recipes") on simulator C2188CC6-…. `bun exact.mjs verify`
and `bun exact.mjs verify ios` were both run: 8/8 tests pass on web and iOS. Light and dark mode
have no findings, nothing spills at the largest accessibility text size on iOS, and scrolling
reports "like UIKit".

## What's in it
- **Recipes tab:** all 160 recipes, each showing its title, category, time and a star rating. The
  bar's search field matches titles and ingredient names. It doesn't match quantities, so typing "2"
  doesn't return everything. A filter pull-down in the bar offers All Categories plus the nine
  categories, and a sort pull-down offers Name, Time (quickest first) and Rating (highest first). Ties
  are broken by name. Filter and search work together. The section header says what is shown, e.g.
  "4 recipes in Pasta · by name". When nothing matches, an empty state appears; if a category is set,
  it has a "Show All Categories" button. The sort choice is kept across launches.
- **Recipe screen:** title, category, time, rating, a servings stepper (UIStepper, 1–99), the
  ingredients and the numbered steps. Quantities scale to the chosen servings and show as cook's
  fractions (1½, 5⅓, ⅔). If no common fraction fits, the amount is rounded to one decimal place. A
  footer says what the quantities were scaled from. A blank or out-of-range entry falls back to the
  recipe's own servings. The heart in the bar saves or removes the favourite.
- **Favorites tab:** saved recipes sorted by name, with search, swipe to remove, and their own
  navigation stack. When empty it shows "No Favorites Yet" with an explanation and a "Browse Recipes"
  button. Favorites are a `persist` state, so they survive relaunching; a test does a `reload` to check this.
- **Data:** `app.ts` imports `data/recipes.json`, so it is bundled into the app. The import splits
  each ingredient into its amount and the rest of the text. Every ingredient in this data starts
  with a whole number; anything without a number would be shown unscaled.

## What was hard / guessed
- Multi-line ternaries in a `fn` have to be wrapped in parentheses. The compiler error says so clearly.
- A grouped-list `row` with two `text`s is drawn as a title plus a grey trailing detail. My
  ingredient and step rows looked wrong until I wrapped the two texts in an inner `row`. I only
  found this from a screenshot; the docs mention it in a single line of the grouped-form recipe.
- An `EmptyState`'s button has to be indented *under* the EmptyState, not placed after it.
  Otherwise the button drops to the bottom of the screen. The `contract vocab EmptyState` entry
  explains this; the card doesn't make it obvious.
- The first `ios --prebuild` started before I had replaced the template, so its bake failed. That
  was harmless, but the stale-failure message showed up again on the next drive.
- `verify` on the web still reports overlaps at the largest text size for one test. That test ends
  with the category menu open, which the web build paints as a popover over the page. On iOS this
  menu is a real UIMenu and iOS verify reports nothing, so I left it.
- Choices I made: a filter pull-down for 10 categories rather than chips (the docs say a segmented
  control is for five options or fewer); a decimal fallback for awkward scale factors; and resetting
  the stepper whenever you navigate.

## Unfinished
Nothing in the spec is missing.

## What would have made it faster
- A pattern that already has a `section` header and a detail screen laid out as a grouped list.
- A note on the card that two `text`s in a row become title and detail.
- A one-line way to see a screen at the largest text size, like
  `agent … "prefer root-font-size 53"`. I had to find it in verify.mjs.
