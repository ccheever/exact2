# Recipes (UIKit) — notes

## What's built
- **Recipes tab**: a `UICollectionView` list (diffable data source) of all 160 recipes. Each row shows the title, then category, time and rating, with a colored category icon and a heart if it's a favorite. The search bar (title **or** ingredient; every word must match, case- and diacritic-insensitive), the Category menu (All + 9 categories, with counts) and the Sort menu (Name A–Z / Time quickest first / Rating highest first, ties broken by name) all work together. The subtitle under the large title shows the current state, e.g. "Soups · 6 recipes · by name". The sort order is remembered between launches. If nothing matches, an empty state offers "Show All Categories". Swipe a row or long-press it (context menu with preview) to favorite or unfavorite.
- **Recipe screen**: an inset-grouped table. Header with category, time, servings, rating and favorite status, then a servings stepper (1–99) with Reset, scaled ingredients and numbered steps. Quantities use kitchen fractions (1½, ⅔, ¾…) and "cup" becomes "cups" when needed. A footer says what the amounts are scaled from. The heart button in the nav bar toggles the favorite. The nav-bar title only appears once the big title has scrolled off.
- **Favorites tab**: newest first. Swipe or Edit to remove. When empty, it shows "No Favorites Yet", explains how to add one, and has a "Browse Recipes" button. Favorites are stored in `UserDefaults` as recipe ids, and I tested that they survive relaunching.
- Works in dark mode and at the largest accessibility text size: the stepper row stacks vertically, and non-breaking spaces keep each icon on the same line as its value.

## What was hard / guessed
- There's no UI-automation tool, so I couldn't tap. I added launch-argument test hooks (`-debugTab`, `-debugRecipe`, `-debugServings`, `-debugCategory`, `-debugSearch`, `-debugToggleFavorite`, `-debugScrollSection`, plus `-sortOrder`) and checked each state with `simctl io screenshot`. They have no effect unless passed.
- `simctl spawn … defaults write <bundle id>` doesn't write into the app's container, so seeding favorites that way didn't work. I tested persistence with the in-app toggle hook across separate launches instead.
- How to scale quantities was my call: the data only has whole leading numbers ("2 tbsp sugar"). Lines without a number are left as they are. The data has odd phrasing like "2 cloves eggs", which I didn't change.
- Nine categories are too many for a search scope bar, so the category filter is a nav-bar menu. Its filled icon shows when a filter is on, and the subtitle names the category.
- The `UINavigationItem.subtitle` API is iOS 26-only (fine here, since the minimum is 26).

## Not done
- `UIStepper` doesn't grow with Dynamic Type, so it stays small at accessibility sizes. Rows reflow around it.
- No XCTest suite (none available in the skeleton). Everything was verified by screenshots.

## What would have made this faster
- A way to tap and type in the simulator (e.g. idb or an XCUITest runner) instead of building launch-argument hooks.
