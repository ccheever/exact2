# Recipes (Exact) — build notes

Installed on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432` as `Recipes` (`dev.bakeoff.recipes.b3128ba`).
`bun exact.mjs verify ios` is clean: 6/6 tests pass, no light or dark findings, nothing spills at
the largest text size, and scrolling is "like UIKit".

## What's in it
- **Recipes tab**: all 160 recipes in an inset-grouped list. Each row shows the title, then
  "category · time", then the rating with a star. The bar's search field matches titles and
  ingredients. A filter pull-down (UIMenu) picks All Recipes or one of the nine categories, and a
  sort pull-down picks Name (A to Z), Time (quickest first) or Rating (highest first). The
  current option has a checkmark. Search and category work together. The section header says
  what is showing ("Soups · 6 recipes"). When nothing matches, an empty state says why and offers
  "Show All Recipes".
- **Recipe screen**: shows the category, time, rating and a Servings field with a UIStepper
  (1–99, and you can type in it too). The ingredients are rescaled as servings change and shown
  as kitchen fractions (¼ ⅓ ½ ⅔ ¾). The footer says "Quantities scaled from N servings". The
  steps are numbered. A heart in the bar adds or removes the recipe from Favorites.
- **Favorites tab**: saved recipes sorted by name, with their own search. You can swipe a row to
  remove it. When empty it says "No Favorites Yet" and explains how to add one, with a "Browse
  Recipes" button. Favorites and the chosen sort order are kept across relaunches (`persist`).
  The test suite checks this with `reload`.
- `data/recipes.json` is bundled by importing it in `app.ts`. That code splits "2 tbsp sugar"
  into an amount and the rest, and gives each ingredient and step its own key.

## What was hard / guessed
- **Fraction display.** Contract has no `abs` or `pow`, so the fractions are snapped with a
  chain of comparisons in 24ths. Amounts below ⅛ show as "⅛". Units aren't pluralised: the data
  itself has lines like "2 cloves eggs", so I left them as written.
- **A row with two `text`s becomes a title and its value** (the value right-aligned and grey).
  The ingredient rows needed an inner `row` to keep the amount and the item together.
- **Where the empty state's button goes.** Putting it as a sibling of `EmptyState` placed it at
  the bottom of the screen. It has to be indented as the part's child.
- **The stepper on iOS** could only be pressed by reading `layout`'s `native.stepper` points and
  using `tap … at x y` with `--touch platform`. I checked it by hand that way (2 → 4 servings
  rescaled correctly). The tests type the value instead.
- **Category control.** Ten options is too many for a segmented control, so I used a bar
  pull-down menu. That fits iOS, but the active filter shows only in the section header and the
  menu's checkmark. A filled filter glyph would need an SF Symbol plus an icon token for
  web/Linux, so I skipped it.

## Not finished / known
- `bun exact.mjs verify` on the **web** reports 13 "overlap" findings at the largest text size,
  all at one moment: when the test has the category menu open. The web draws that menu as a
  popover over the page, so it covers content at 53 px text. On iOS the menu is UIKit's, and
  `verify ios` is clean. I left the finding unfixed.

## What would have made it faster
- An `exact.mjs` command that doesn't hang silently while the first compiler build and the iOS
  prebuild compete. My first `contract vocab` call sat for minutes with no output.
- `contract vocab stepper` could show the iOS tap-at-point recipe as a test step, so the
  stepper itself can be covered in a test.
