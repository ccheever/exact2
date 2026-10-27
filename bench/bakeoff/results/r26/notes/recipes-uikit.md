# Recipes (UIKit) — build notes

## What's there
- `Sources/App.swift` – UIKit app/scene delegates (scene manifest in `EXTRA_PLIST`; the scene delegate is
  `@objc(SceneDelegate)` so the plist name doesn't depend on the Swift module name), tab bar (Recipes, Favorites).
- `Sources/Model.swift` – JSON loading (`Resources/recipes.json`, copied from `data/`), filtering/search/sort,
  `FavoritesStore` (UserDefaults, posts a notification), ingredient quantity parsing and scaling.
- `RecipeListViewController` – large title with a count subtitle, search controller (title or ingredient; every
  word must match; case/diacritic-insensitive), category chips (All + 9) in the table header, and a nav-bar menu
  for sort (Name / Time / Rating, persisted) and category. Filter and search combine. Empty state with
  "Search All Categories". Swipe or long-press to favorite.
- `RecipeDetailViewController` – category, title, time/servings/rating tiles, servings stepper (1–48) that rescales
  ingredient quantities (shown as kitchen fractions, e.g. 2½, ⅓), numbered steps, heart button to save/remove.
- `FavoritesViewController` – most-recent-first list, swipe/Edit to remove, `UIContentUnavailableConfiguration`
  empty state with a "Browse Recipes" button. Favorites persist across relaunch (checked by seeding saved IDs in UserDefaults and relaunching; the add/remove path writes the same key).
- Dark mode, the largest accessibility text size (layouts switch to vertical), VoiceOver labels.

## What was hard / guessed
- No tap automation on the simulator, so I added `-debug*` launch arguments (`-debugQuery`, `-debugCategory`,
  `-debugSort`, `-debugRecipe`, `-debugServings`, `-debugSection`, `-debugTab`) read from UserDefaults' argument
  domain, and screenshot them with `tools/shot.sh`. Normal launches ignore them.
- Self-sizing cells that change layout at accessibility sizes: views built before they're in the hierarchy see
  default traits, so the header tiles were measured with the wrong axis. Fixed by deciding the layout from the
  cell's traits in `configure` and reloading on content-size changes.
- Guesses: ingredient quantities are a leading number (data only has 1–4, but fractions/decimals/unicode are
  parsed too); lines without a number aren't scaled. Units aren't pluralised ("10 cup") since the source data
  doesn't pluralise either. Ratings are out of 5. Search matches ingredient names, not their quantities.
- `UINavigationItem.subtitle` (iOS 26) is used for the result counts.

## Not done
- Interactive gestures (swipe, stepper taps, menu) were verified by code review only, not by driving them on the
  simulator — the tooling had no way to tap.

## What would have helped
- A way to send taps/typing to the simulator (or XCTest UI tests) to exercise interactions end to end.
