# Recipes (Exact): notes

## What's built
- **Recipes tab:** all 160 recipes from `data/recipes.json`, which `app.ts` imports, so it is bundled with the app. Each row shows the title, then "category · time" (for example "1 hr 20 min"), and a star with the rating on the right.
  - The navigation bar has a search field. It matches the title or any ingredient.
  - Two bar buttons open UIKit pull-down menus: Category (All Recipes plus the nine categories, with a checkmark on the current one) and Sort (Name, Time, Rating).
  - Category, search and sort all apply together. A section header shows the current state, for example "Soups · 16 recipes by name".
  - When nothing matches, an empty state offers "Show All Recipes", which clears the category and the search.
  - The sort choice is kept across launches.
- **Recipe screen:** category, time and rating, plus a servings row with the native UIStepper.
  - Ingredients scale to the chosen servings and are rounded to cook's fractions (½, ⅓, ¼, ⅛, …).
  - The footer below the summary says what the recipe makes, or what the ingredients were scaled from.
  - Steps are numbered.
  - A heart in the bar adds the recipe to Favorites or removes it.
- **Favorites tab:** saved recipes sorted by title, with their own search. Swipe a row to remove it.
  - When empty, it shows "No Favorites Yet" with a "Browse Recipes" button.
  - Favorites are a `persist` state (a list of ids), so they survive relaunch; a test covers this with `reload`.
- **Tests:** 5 in `app.test.contract`. They pass on web and on iOS, and `verify` / `verify ios` show no findings for light, dark, or the largest text size.

## What was hard, or guessed
- **Naming clashes:** actions called `filter` and `sortBy` shadow the built-in methods, which gave confusing compile errors at first.
- **Typing an empty state:** an empty list state needs a type (`state x: string[] = []`).
- **Servings stepper on iOS:**
  - The number pad has no Return key, so `change` never fires on Enter.
  - UIStepper shows "1" when you type "0", while the state still holds "0".
  - Fix: scaling clamps whatever is typed to 1–99 on every keystroke, so the field and the quantities always agree. An empty field falls back to the recipe's own servings.
- **Steps layout:** in a grouped row, two texts read as label and value, so the step text came out right-aligned and grey. Wrapping both texts in an inner `row flex=1` fixed it.
- **Ingredient data:** the lines have no real units (for example "2 cloves eggs", "1 cumin"). I scale the leading number and leave the rest of the text alone, as written.
- **Category order:** I guessed it should be alphabetical; the nine categories are a fixed list in the view.

## Not finished, or open
- **Scrolling findings:** `verify ios` reports scrolling differences from UIKit on the 160-row list: an occasional hitch, a tracking ratio of 0.93, and offset jumps while decelerating. The count varies by run: the first run on the same rows had only the tracking finding. The rows are already light (two texts, a star, a rating and a chevron), so Exact's own report says the cause is Exact, not the app.
- **Web-only findings:** web `verify` reports overlaps at the largest text size. They appear only while the category pull-down is open in the web build, where it is a painted popover drawn over the page. iOS shows it as a UIMenu instead.
- **Agent drags:** I couldn't drive real-touch drags with `agent ios --touch platform` (simulator accessibility didn't respond). I checked the lower part of the detail screen in a tall web window instead.

## What would have made it faster
- A recipe for "a numbered list of paragraphs in a grouped list", or a warning when a row's second text is about to become a trailing value.
- A note in the `stepper` vocab that Enter does nothing on the iOS number pad, so a test must not depend on `change` from `key "Enter"`.
- An `agent ios` way to scroll a list to a testId (`scroll-to <id>`) without real touches.
