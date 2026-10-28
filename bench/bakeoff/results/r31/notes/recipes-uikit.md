# Recipes (UIKit) — notes

## What's in it
- UIKit app delegate and scene delegate, declared in `EXTRA_PLIST`. There's a `UITabBarController` with two tabs, Recipes and Favorites, and each tab has its own navigation controller.
- **Recipes list:** a plain table using a diffable data source. Each row shows title, category, time and rating, and a heart if the recipe is a favorite. The search bar matches titles and ingredients; every word you type must match somewhere, ignoring case and accents. A sticky row of category chips sits above the list (All plus the 9 categories in the data). Search and category filter together. The Sort menu offers Name, Time (shortest first) and Rating (highest first), with ties broken by name, and the choice is remembered. The empty state shows "No Results" with a "Show All Categories" button when a category is narrowing the results. You can swipe a row or long-press it (context menu with preview) to add or remove a favorite.
- **Detail screen:** a large wrapping title, the category, and time, servings and rating. Below that are a servings stepper (1–99), the ingredients and numbered steps. Ingredient quantities are scaled and shown as kitchen fractions (½, ⅓, ¾ …). Very small amounts are never rounded down to 0; they show at least ⅛. Amounts of 10 or more round to whole numbers. The heart button in the nav bar toggles the favorite, and the title moves into the nav bar once you scroll past it.
- **Favorites tab:** lists favorites in the order they were saved. It has an Edit button for deleting and reordering, and swipe-to-remove. When empty it shows `UIContentUnavailableConfiguration` with an explanation and a "Browse Recipes" button. Favorites are stored in `UserDefaults` as an id array; stale ids and duplicates are dropped on load.
- **Dark mode and Dynamic Type:** only system colors are used. Rows wrap at accessibility sizes, and the stat and servings rows switch to a vertical stack at those sizes.

## What was hard or guessed
- There's no tap or UI-automation tool, so I added launch-argument hooks (`Sources/DebugHooks.swift`: `-demoCategory`, `-demoQuery`, `-demoSort`, `-demoOpen`, `-demoServings`, `-demoTab`, `-demoToggleFavorite`). I drove each screen with these and checked screenshots (`shot.sh`) in light, dark and AX-XXXL text. They do nothing unless the arguments are passed.
- Getting the horizontal chip bar to size itself inside a table section header took two tries. Chips first stretched to equal widths, then got squashed to one letter per line. The fix was to size the scroll view's height from the stack, not the other way round.
- The ingredient strings are all "<integer> [unit] name", so I parse only a leading number or fraction. Text with no number is shown unscaled.
- I guessed the sort directions: rating descending, time and name ascending.

## Not verified / not finished
- Real tap gestures were never exercised: swipe actions, the stepper, the Edit/reorder mode, the context menu and live search typing. I checked them through code review and the launch hooks only.

## What would have made this faster
- A way to tap and type on the simulator (e.g. idb or an XCUITest runner) for real interaction tests.
