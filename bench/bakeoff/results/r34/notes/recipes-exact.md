# Recipes: build notes

The app is built with Exact (started from the `tabs` pattern) and installed on simulator
`C2188CC6-FBE9-409A-92A8-ABD89AC2C432` as `dev.bakeoff.recipes.bbe909c` / "Recipes".
All 6 tests pass. `verify` passes, and `verify ios` is clean: light, dark, the largest text
size ("nothing spills") and scrolling ("nothing for the app to fix").

## What's in it
- **Recipes tab:** all 160 recipes from `data/recipes.json`. `app.ts` imports the file, so it
  is bundled with the app. Each row shows the title, then "category · time", then "★ rating".
  - Search (UIKit's search bar) matches titles and ingredient names, ignoring case.
  - A filter menu in the bar picks a category: All plus the nine.
  - A sort menu picks Name A–Z, Time (quickest first) or Rating (highest first). Ties fall
    back to name order.
  - Both menus are native UIMenus with checkmarks. Search, category and sort all combine.
  - The section header shows the active category and the count ("Soups · 5 Recipes").
  - When nothing matches, an empty state says what was searched and offers "Show All Recipes".
  - The chosen category and sort are kept across launches.
- **Recipe screen:**
  - Category, time ("1 hr 20 min"), rating, and a servings field with UIStepper (1–99).
  - Ingredients scale with the servings. Quantities read the way a cook writes them: whole
    numbers, then ¼ ⅓ ½ ⅔ ¾, otherwise one decimal (3 tsp for 4 → "1½ tsp" for 2,
    "2¼ tsp" for 3).
  - A footer says "Scaled for N servings from M." If the field is cleared or holds an
    invalid number, the recipe's own servings are used.
  - Steps are numbered.
  - A heart in the bar adds the recipe to Favorites or removes it, with a success haptic
    when adding.
- **Favorites tab:**
  - Lists saved recipes by name, with its own search.
  - Swipe a row to remove it, or use the heart on the recipe screen.
  - When empty it shows "No Favorites Yet: open a recipe and tap the heart…" with a
    "Browse Recipes" button.
  - Favorites are a `persist` state (UserDefaults), so they survive a relaunch. A test
    covers this with `reload`.

## What was hard / what I guessed
- **Rows in a grouped list read the second `text` as a trailing grey detail.** My numbered
  steps first came out right-aligned and grey. Wrapping the number and step in an inner
  `row` fixed it. I only found this from a screenshot, not from the docs.
- **The detail rating row squeezed titles at the largest text size.** Switching to the
  `RowText` part fixed it, since it moves the value under the title when it doesn't fit.
  The cost is that the star is now a "★" glyph instead of an orange SF Symbol.
- **Web `verify` flags the open sort menu as "overlap" at the largest text size.** That is
  the web-painted popover sitting over the list, which is how a menu should look.
  I checked a screenshot and left it. iOS uses a real UIMenu and reports nothing.
- **The servings field's `change` handler fired after its screen was popped.** `verify ios`
  logged it as "UnknownView". Removing `change` and keeping `input` fixed it.
- **Two guesses:**
  - Sorting by rating goes highest first and by time goes quickest first, since that is
    what someone browsing wants.
  - Each ingredient line is parsed as "leading number + rest", and the number is scaled.
    The data only has whole-number quantities.
- **There's no `abs()` in Contract, so I wrote `max(a - b, b - a)`.** I also had no `%`
  operator to rely on, so minutes use `floor`.
- **`symbol:filter-fill` isn't a role.** The SF variant draws blank on the web, so the filter
  button keeps one icon, and the active category shows in the section header instead.

## Not finished / caveats
- `verify ios` exits 1. Its report says "verify: clean". The exit comes from the toolkit's
  own two-engine semantics check (unsupported `toLowerCase`, one shrink candidate it filed
  under the pins directory), not from an app finding.
- The app ships iPhone only (`deviceFamily: ["iphone"]`), and I removed Linux with
  `update --without linux`. I didn't do a two-column iPad layout.

## What would have made this faster
- A doc note that a list `row` with two texts lays out as title + detail, and how to opt out.
- A built-in way to scroll a list in `agent ios` that was easier to find. I had to add a
  `testId` to the list and use `tap <list> drag 0 -400`.
- A way to mark an open popover as expected in the web large-text check.
