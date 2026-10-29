# Recipes (UIKit) — notes

## What's built
- UIKit, written in code: an app delegate and scene delegate (the scene manifest is in `EXTRA_PLIST`), and a tab bar with Recipes and Favorites.
- **Recipes:** a compositional-layout collection view.
  - The first section is a horizontally scrolling row of category chips (All plus the 9 categories in the data). The second section is the recipe list: title, category icon, category, time, rating, and a heart on favorites.
  - The search controller matches titles and ingredient names. Every word typed must match either the title or an ingredient, and the search combines with the category filter. When a recipe matches on an ingredient, its row shows "Contains …".
  - A Sort menu (Name / Time / Rating, with a checkmark on the current one) is saved between launches. Ties are broken by name.
  - When nothing matches, the list shows a message that names the search and category. The chips stay visible so you can widen the filter.
  - Swipe right on a row, or long-press it, to add or remove a favorite.
- **Detail:** an inset-grouped table.
  - A summary card shows the category, title, time, servings and rating.
  - A servings stepper (1–99) has a "Reset" button and an "Original recipe serves N" note.
  - Ingredients scale with the servings. Amounts show as kitchen fractions (1½, ⅓, ¾), and grams as decimals. "cup/cups" and "clove/cloves" change with the amount.
  - Steps are numbered with circled-number symbols.
  - A heart button in the navigation bar saves or removes the favorite.
- **Favorites:** most recently saved first. Swipe or long-press to remove a recipe. When the list is empty, the system empty-state view explains how to save recipes and has a "Browse Recipes" button.
  - Favorites are stored in UserDefaults as a list of recipe ids. Ids missing from the data are dropped on load.
  - A notification keeps all three screens in sync.
- Uses system colors throughout (checked in dark mode) and Dynamic Type throughout. At accessibility sizes, list rows drop the category icon and the detail stats and stepper stack vertically. VoiceOver labels are set on rows, stats, the stepper and the heart button.

## What was hard / gotchas
- On launch, the app crashed with "Could not cast NSNull to NSString". The cause was a `CellRegistration` whose item type was `String?`: the `nil` used for "All" is bridged to `NSNull`. Registering with the enum item type fixed it.
- The skeleton has no tool for tapping the simulator. I added launch arguments (`-demoQuery`, `-demoCategory`, `-demoSort`, `-demoRecipe`, `-demoServings`, `-demoTab`, `-demoFavorite`, `-demoClearFavorites`) so each state can be set up and screenshotted with `simctl launch`. They do nothing unless passed. `shot.sh` wraps this.
- Not tested, because there is no way to tap or swipe: live typing, the swipe actions and the stepper buttons. Their underlying code paths were exercised through the launch arguments.

## Guesses
- Ingredient lines look like "qty [unit] name". The parser also accepts fractions, unicode fractions and mixed numbers ("1 1/2"). Lines with no leading number are shown unscaled.
- Favorites are ordered newest first. The servings range is 1–99.

## What would have made this faster
- A simulator tap/typing tool (or idb) for real interaction testing.
