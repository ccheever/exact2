# Recipes (UIKit) — notes

## What's there
- `UITabBarController` with Recipes and Favorites tabs, each in a navigation controller with large titles.
- Recipes list: diffable `UITableView`. There's a stacked `UISearchController` that matches titles or ingredient
  names (case and accent don't matter, and every word must match). Below it is a scrolling row of category chips
  (All + the 9 categories). Search and category filter together. The sort menu (Name / Time / Rating, ties broken
  by name) is in the nav bar and remembers your choice. When nothing matches, an empty-results view offers
  "Search All Categories". You can swipe or long-press a recipe to add or remove it from Favorites.
- Detail: a header with category, title, and time/servings/rating tiles. Then a servings stepper (1–99), scaled
  ingredients and numbered steps. A heart button adds or removes the favorite, and there's a share button.
  Scaled amounts show as kitchen fractions (`4½ tsp`, `⅔ cup`). Metric units show as decimals (`1.5 g`). An
  ingredient line with no leading number is shown unchanged.
- Favorites: sorted by name. Swipe or Edit removes one. When the list is empty it shows a message with a
  "Browse Recipes" button. Favorites are stored in `UserDefaults`, so they survive relaunching the app.
- Supports Dynamic Type up to AX5: the stat tiles and the stepper row stack vertically. Dark mode uses a
  dynamic orange tint.

## Hard / guessed
- The data's ingredients are all "N [unit] thing", and some don't make sense (e.g. "2 cloves eggs"). I parse a
  leading quantity (integer, decimal, `a/b`, unicode or mixed fractions) and a unit from a known list, and leave
  the text unchanged when there's no quantity. I don't pluralize units, because the source data doesn't.
- The category chips wouldn't scroll to the selected chip on first appearance. The chip row is a table header,
  and its scroll view's content size isn't set yet at that point. I fixed it by deferring the scroll to the next
  run loop after layout.
- The table header's margins didn't line up with the inset-grouped cells, so I pinned the header to the safe
  area with a fixed 20 pt inset instead.

## Testing
There's no UI automation, so I used launch arguments to put the app into specific states (`-debugRecipe`,
`-debugServings`, `-debugTab`, `-debugSearch`, `-debugCategory`, `-debugSort`, `-debugFavorite`) and took
screenshots with `./shot.sh`. Normal launches ignore these arguments.

I checked these states: the list, search + category together, the no-results state, sorting by rating, the
detail screen scaled to 6 servings, empty and filled Favorites, Favorites after a relaunch, and dark mode at
AX5.

I did not check these: actual taps and swipes, or the stepper being pressed. I tested them only by reading the
code.

## What would have made it faster
A way to tap or type in the simulator (idb or XCUITest) would have let me test the real interactions instead
of using launch-argument hooks.
