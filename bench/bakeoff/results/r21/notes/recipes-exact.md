# Recipes (Exact) — notes

## What was built
- **Recipes tab**: all 160 recipes from `data/recipes.json`. `app.ts` imports the JSON, so it is bundled into the app. Each row shows the title, then category · time · ★ rating, plus a heart if the recipe is a favorite.
  - The bar's search field matches titles **and** ingredient names.
  - A **Filter** menu has All Categories plus the nine categories (radio checkmarks). A **Sort** menu offers Name A–Z, Time quickest first, or Rating highest first.
  - Filter, search and sort combine. A summary line says what is shown, e.g. "Soups · 16 recipes · by Name".
  - When nothing matches, an empty state explains why and has a "Show All Recipes" button that resets the filters.
- **Recipe screen**: title, category, time, rating, and a Servings stepper (UIStepper plus an editable 1–99 field).
  - Ingredient quantities rescale live and are written the way a cook would: fractions such as 1½ and ⅓, and singular/plural units (1 cup / 2 cups, 1 clove / 3 cloves).
  - A footer says "Scaled from 2 servings to 3 servings." A bad or empty value goes back to the recipe's own servings when you press Enter or Done.
  - Steps are numbered. The heart in the bar adds or removes the recipe from Favorites.
- **Favorites tab**: saved recipes, swipe to remove. When it is empty it says "No Favorites Yet" with a hint and a "Browse Recipes" button.
- Favorites (and the chosen sort) are saved with `state … persist` (UserDefaults), so they survive a relaunch. A test relaunches and checks this.
- 6 tests in `app.test.contract`. `verify` (web) and `verify ios` both pass: light, dark, and the largest text size with nothing spilling.

## What was hard / guessed
- **The bundle id.** Exact's `app.schema.json` requires each part of a reverse-DNS id to start with a letter, so it refuses `dev.bakeoff.recipes.40be31` (Apple itself accepts it). I couldn't edit the pinned toolkit, so I worked around it:
  - I built and tested as `dev.bakeoff.recipes.x40be31`.
  - At the end I copied the installed `.app` to `.exact/final/` and set `CFBundleIdentifier` (and `receipt.json`) to `dev.bakeoff.recipes.40be31`.
  - Then I re-signed it ad hoc, uninstalled the placeholder, installed the copy, and launched it. The simulator now has only `dev.bakeoff.recipes.40be31`.
  - Consequence: running `bun exact.mjs ios` again will install the `x40be31` build alongside it, and you'd have to repeat the repackaging step.
- **Ingredient strings** are of the form "qty [unit] name", with units cup/tsp/tbsp/g/cloves. Some have no unit ("1 cumin"). I parse them in `app.ts`. For very small scaled amounts I fall back to one decimal place.
- **Typing "0" in the servings field** behaves differently per platform: iOS clamps it to the stepper's minimum of 1, while the web keeps the text. The test uses an empty value instead.
- **Small fixes:**
  - An action named `sortBy` shadowed the standard function of the same name, so I renamed it.
  - A view can't contain `let`, so I moved that logic into a `fn`.
  - `font-weight` takes a number, not `bold`.
  - At the largest text size the servings field overflowed until I wrapped its row and gave it `flex-basis`.
- **Web-only warnings:** the web largest-text check reports the open category menu overlapping the content. That is the web popover drawn over the page; on iOS it is a UIKit menu and `verify ios` is clean.

## Not done
- No iPad/two-column layout (`deviceFamily` is iPhone only).

## What would have made it faster
- An Exact schema that accepts any valid Apple bundle id, or a separate `host.ios.bundleId` override.
- A view-level `let` for values used several times in one screen.
