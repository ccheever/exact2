# Recipes (UIKit): notes

## What's there
- A tab bar with Recipes and Favorites. Everything is built in code with UIKit: an app delegate and a scene delegate, with the scene manifest in `EXTRA_PLIST`.
- **Recipes list:** each row shows the title, category, time and rating, and the text wraps at large sizes.
  - A search bar matches titles or ingredient names. It ignores case and accents, and every word you type must match.
  - A scrolling row of category chips (All plus the nine categories) filters the list. The chips and search work together.
  - A sort menu (Name, Time or Rating) stays the same after relaunch. Ties are broken by title.
  - When a search matches only by ingredient, the row says why ("Contains basil").
  - Empty results show a "no results" screen, with a "Show All Categories" button when a category is selected.
  - Swipe a row or long-press it (context menu) to add or remove it from Favorites. Favorited rows show a heart.
- **Recipe screen:** a large wrapping title with the category, then time, rating, a servings stepper (1–50) with a Reset button, ingredients and numbered steps.
  - Quantities scale with the stepper and show as fractions (e.g. 2 × ¾ → 1½). Values that aren't a common fraction fall back to one decimal place.
  - The footer says what the quantities are scaled for.
  - The heart button in the navigation bar adds or removes the favorite.
- **Favorites:** most recently added first. You can remove items by swiping or with Edit.
  - When empty it shows "No Favorites Yet", explains how to add one, and has a "Browse Recipes" button.
  - Favorites are saved in UserDefaults as recipe ids. IDs that aren't in the data are dropped when the app loads.
- Works in dark mode (system colors only) and at accessibility text sizes: the servings row stacks vertically, and value cells stack.

## What was hard / guessed
- The simulator can't be tapped from the command line, so I added screenshot launch arguments: `-shotTab`, `-shotCategory`, `-shotQuery`, `-shotOpen <id>`, `-shotServings`. They set the app to a given state through UserDefaults' argument domain, and normal launches ignore them. I checked each state with `simctl io screenshot`. Persistence was checked by writing the defaults key with `simctl spawn defaults` and relaunching. Tapping-only paths (stepper taps, swipe actions, the heart button) were never run; I only reviewed that code.
- Ingredient strings look like "2 cloves eggs" and "1 cumin", always an integer quantity first. I treat the first token as the quantity and match search against the name with a known unit word removed. Unit words aren't made plural or singular when scaling (e.g. "1½ cup").
- There are ten categories, too many for a search-bar scope bar, so I used a chip row as the table header. It's re-sized by hand when the text size changes.

## Not done
- No XCTest/UI tests (the toolkit has no test runner).

## What would have made this faster
- A way to send taps or gestures to the simulator (or idb) to test interactions end to end, instead of using launch arguments.
