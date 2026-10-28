# Recipes (UIKit): notes

## What's built
- **Recipes tab**: a list (compositional-layout list, diffable data source) with title, category, time and rating on each row. A row of category chips (All plus the nine categories) stays pinned above the list. The search controller matches titles and ingredients, ignoring case and accents, and every word must match. Category and search apply together. The Sort menu (Name, Time, Rating) uses single-selection checkmarks; ties are broken by name, and the chosen sort is remembered between launches. When nothing matches, a "No Results" view appears below the chips, with a "Search All Categories" button.
- **Recipe screen**: an inset-grouped table with category, title, rating, total time and a Servings stepper (1–99), then the ingredients and numbered steps. Changing servings rescales the leading quantity of each ingredient, shown as a friendly fraction (1½, ⅔, …), and a footer notes the original serving count. The heart button in the nav bar saves or removes the recipe. The nav-bar title only appears once the large header title has scrolled away.
- **Favorites tab**: saved recipes, newest first. Swipe to remove. When empty it shows a heart, an explanation and a "Browse Recipes" button. The Recipes list also has a swipe-to-favorite action and a context menu, and favorited rows show a heart.
- **Persistence**: favorites are saved as JSON in Application Support with an atomic write on every change. I checked this across relaunches (terminate, then launch).
- Dark mode, Dynamic Type up to AX5 (the row metadata stacks vertically; the servings row stacks the stepper below), VoiceOver labels and values on rows and steps, and haptics.

## Testing
There is no UI automation, so I added `-Demo…` launch arguments (see `MainTabBarController.viewDidAppear`) that select a tab, set the category, search, sort or servings, open a recipe, or toggle a favorite. Each state was checked with `simctl io screenshot`, in light and dark mode and at the default and AX5 text sizes. The arguments do nothing on a normal launch.

## What was hard / guessed
- Self-sizing rows built from a custom stack view sometimes came out with a gap of about one line (a multi-line label measured at the wrong width). Switching to `UIListContentConfiguration.subtitleCell`, with an attributed secondary line containing SF Symbol attachments that is rebuilt in `updateConfiguration(using:)`, fixed it, and it also follows Dynamic Type.
- `collectionView.backgroundView` didn't show the empty state under a pinned supplementary header, so I used an overlay `UIContentUnavailableView` placed below the chip bar.
- In the data, quantities are always a leading integer and units are odd ("2 cloves eggs", "1 cumin"). Scaling only touches the leading number and leaves the wording alone, so units and plurals are not adjusted. The parser also accepts decimals, "1/2", "1 1/2" and Unicode fractions.
- I guessed the rest: 99 as the servings maximum, newest-first order for Favorites, and the orange accent colour.

## Not done
- No app icon, and no undo after removing a favorite.

## What would have made this faster
- A way to tap and type in the simulator (or XCUITest support in the build script). Without it, every interaction had to be faked through launch arguments.
