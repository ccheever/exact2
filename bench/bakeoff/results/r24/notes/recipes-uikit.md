# Recipes (UIKit) — notes

## What was built
- Programmatic UIKit app: `AppDelegate` + `SceneDelegate` (scene manifest in `app.env` `EXTRA_PLIST`), `UITabBarController` using `UITab` (Recipes, Favorites).
- **Recipes list**: `UICollectionView` with a compositional layout. Section 0 is a horizontally scrolling row of category chips (All + the 9 categories, built from the data); section 1 is a list. `UISearchController` searches titles **and** ingredients (case- and diacritic-insensitive; every word typed must match somewhere), and it combines with the category chip. A sort pull-down menu in the nav bar (Name A–Z / Time quickest / Rating highest; ties broken by title) is remembered between launches. A nav-bar subtitle shows the count, category and sort. If nothing matches, an inline empty state explains why and offers "Show All Categories".
- Rows show a category icon, title, category, time ("1 hr 20 min") and ★ rating, plus a heart when the recipe is a favorite. Swipe or long-press a row to add or remove a favorite.
- **Detail**: an inset-grouped list with an overview card (category, title, time, servings, rating), a servings stepper (1–99), ingredients scaled to the chosen servings, and numbered steps. Counted amounts are shown as kitchen fractions (½, ⅓, ¾, ⅛ …). Gram amounts are shown as decimals to one place. An amount is never shown as zero. A footer says the amounts are scaled from the original servings. The heart button in the nav bar toggles the favorite.
- **Favorites**: most recently saved first. Swipe to remove. When the list is empty, a `UIContentUnavailableConfiguration` says how to add favorites and has a "Browse Recipes" button that switches tabs. Favorites are stored in `UserDefaults` as an ordered array of ids. On load, ids that are unknown or duplicated are dropped.
- Dynamic Type everywhere. At accessibility sizes the stats in the detail card stack vertically. Dark mode uses system colors. VoiceOver labels are set on rows, stats, steps, the stepper and the heart.

## Testing
I had no tool to tap the simulator, so I added launch arguments that drive the app to a given state: `-open <id>`, `-servings n`, `-steps YES`, `-tab favorites`, `-search`, `-category`, `-sort`, `-favoriteIt YES`, `-clearFavorites YES`. `tools/shot.sh NAME args…` relaunches the app with those arguments and saves a screenshot to `shots/`. With these I checked the list, combined search and filter, the empty search result, sorting, scaling, steps, favorites persisting across relaunch, the empty Favorites screen, dark mode and AX-XXXL text. Taps, swipes and the keyboard were not exercised directly.

## What was hard / guessed
- Diffable `CellRegistration<ChipCell, String?>` crashed at runtime (NSNull cast to NSString) because of the optional item type. I registered the cell with the enum item type instead.
- A `UIStepper` used directly as a custom cell accessory was clipped at the trailing edge on iOS 26. Wrapping it in a container view sized with Auto Layout (`maintainsFixedSize`) fixed it.
- `xcrun simctl spawn … defaults` could not see the app's defaults domain, which is why the `-clearFavorites` hook exists.
- Guesses: the data's quantities are all whole numbers, so fractions were picked for scaling. Unit words are left as written (e.g. "2 cloves eggs" stays as is), because the data is synthetic. The SF Symbol for each category is my own choice, with a fallback if a symbol is missing.

## Not done
- No iPad or landscape layout (the skeleton's plist is portrait iPhone only).
- No UI tests; there is no XCTest in this toolchain.

## What would have made this faster
- A way to tap and swipe on the simulator (e.g. idb or an XCUITest runner) so interaction could be tested without launch-argument hooks.
