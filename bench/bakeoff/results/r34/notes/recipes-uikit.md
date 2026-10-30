# Recipes (UIKit): notes

## What's built
- `UITabBarController` with two tabs, **Recipes** and **Favorites**. Each tab has a navigation controller with a large title.
- **Recipes tab**: a compositional-layout `UICollectionView` with a diffable data source.
  - A row of category chips that scrolls sideways: All plus the nine categories.
  - The recipe list, with a sticky header showing the count and the sort order ("42 Recipes · Rating").
  - `UISearchController` searches titles and ingredient names. Matching ignores case and accents, and every word you type must match. Search and category filter combine.
  - A sort menu offers Name (A to Z), Time (quickest first) and Rating (highest first). Ties fall back to name. The choice is saved in UserDefaults.
  - When nothing matches, a "No Recipes Found" cell explains why and offers a "Clear Search and Filter" button. It is a cell rather than an overlay, so the chips stay usable.
  - Swipe a row or long-press it to add or remove a favorite.
- **Detail screen** (inset-grouped table):
  - Header: category, the title in a large wrapping font, tiles for time, servings and rating, and a Save to Favorites / Saved to Favorites button. There is also a heart in the nav bar.
  - Servings `UIStepper` (1–48) with a Reset button. The footer says what the quantities were scaled from.
  - Ingredients are scaled live. Amounts use kitchen fractions (⅓, ½, ¾ …), grams use decimals, and cup/clove switch between singular and plural.
  - Numbered steps.
- **Favorites tab**: newest saves first. Swipe to remove. When empty, a `UIContentUnavailableConfiguration` explains how to add favorites and has a "Browse Recipes" button.
- **Persistence**: favorite IDs (an ordered `[Int]`) are kept in UserDefaults. I checked that they survive terminating and relaunching the app.
- **Dark mode**: semantic colours only. Checked on the simulator.
- **Largest accessibility text size**: checked on the simulator.
  - List rows drop the icon and trailing rating and move the rating into the subtitle, so titles don't get squeezed letter by letter.
  - The detail stat tiles stack vertically as compact rows.
  - The servings row stacks vertically.
- **VoiceOver**: labels and values on cells, stats, chips (with the selected trait) and the stepper.

## What was hard / guessed
- **Crash on first run**: `Could not cast NSNull to NSString`. The cause was a cell registration typed `CellRegistration<_, String?>`: a `nil` item is bridged through Objective-C as `NSNull`. I fixed it by registering on the enum item type.
- **Data format**: ingredients look like `"2 cloves eggs"` / `"3 tomato"`, with an integer quantity, an optional unit and a name. The data is nonsensical in places ("cloves eggs"). I parse it rather than rewrite it, and only singularise or pluralise cup and clove.
- **Tiny scaled amounts**: these are rounded to the nearest common fraction and never shown as 0; the minimum is ⅛.
- **No tap automation**: only `simctl` was available, with no idb or XCUITest. To screenshot the screens I added launch arguments: `-debugRecipe N`, `-debugServings N`, `-debugScroll N`, `-debugCategory X`, `-debugQuery X`, `-debugTab N` and `-debugToggleFavorite YES`. `build.sh` doesn't define `DEBUG`, so these stay in the shipped build, but they do nothing unless those arguments are passed.
- **Tested only through those launch arguments**: stepper taps, swipe actions and the sort menu are standard controls.

## What would have made it faster
- A UI-driving tool (tap or type on the simulator) or XCUITest support in the build script.
- A `DEBUG` flag in `build.sh`.
