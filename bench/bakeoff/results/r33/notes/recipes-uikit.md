# Recipes (UIKit): notes

## What's there
- A UIKit app built in code: `AppDelegate`/`SceneDelegate` (the scene manifest is in `EXTRA_PLIST`) and a tab bar with
  **Recipes** and **Favorites**, each in a navigation controller with a large title.
- **Recipes**: a compositional-layout collection view. The first section is a row of category chips that scrolls
  sideways (All plus the nine categories). Below it is a plain list section whose header shows the count and sort
  order ("42 Recipes · By Time"). Each row shows the title, category, time and rating, with a heart if the recipe is a
  favorite. The search controller matches titles and ingredients, ignoring case and accents, and every word you type
  must match. Search and category filter work together. The sort menu (Name A→Z, Time quickest first, Rating highest
  first, ties broken by name) is remembered across launches. Swipe a row or long-press it to add or remove a favorite.
  Dragging the list dismisses the keyboard. When nothing matches, the list shows an inline "No Results" message with a
  "Search All Categories" button, and the chips stay usable.
- **Detail**: an inset-grouped table. The header shows the category, a large wrapping title, and tiles for time,
  servings and rating. Below it is a servings stepper (1 to 99) that rescales ingredient amounts in place. The footer
  says what the amounts were scaled from. Ingredients are listed with the amount in bold, and steps are numbered with
  badges. The heart in the navigation bar toggles the favorite. The navigation bar title only appears once the big
  title has scrolled away.
- **Favorites**: stored in `UserDefaults` as recipe ids (newest first) and kept in sync everywhere through a
  notification. Swipe to remove. When empty, it shows a `UIContentUnavailableConfiguration` with a heart, an
  explanation and a "Browse Recipes" button.
- **Scaling** (`IngredientScaler`): reads a leading amount in any of these forms: `2`, `1.5`, `1/2`, `1 1/2`, `½`,
  `1½`. It multiplies by new ÷ original servings and shows common fractions (½ ⅓ ¼ ⅛ ⅙ …). Amounts with no close
  fraction are shown to one decimal place. Lines without an amount are left unchanged.
- Uses system colours and Dynamic Type throughout. At accessibility text sizes the row details and the detail tiles
  stack vertically. VoiceOver labels are combined for rows, tiles and steps.

## Tested on the simulator
I relaunched with launch arguments and took screenshots (`tools/shot.sh`). I checked: the list, combined
search + category + sort, the no-results state, detail scaled from 2→5 servings (2½, 7½ …), the empty and filled
Favorites tab, favorites surviving a relaunch, dark mode, and AX5 text size on the list and the detail screen. I also
ran a small macOS check of the scaler on its own.

## What was hard or surprising
- The app crashed at launch: a cell registration typed `CellRegistration<_, String?>` gets a `nil` item bridged to
  `NSNull`, which crashes with "Could not cast NSNull to NSString". The fix was to register using the item enum
  instead.
- A regex bug: `\d+\s*[½…]?` let the amount token capture a trailing space, so `Double("2 ")` failed and nothing
  scaled. The screenshot caught it.
- Multi-line labels inside a horizontal `UIStackView` with `.firstBaseline` alignment truncated at AX sizes;
  `.top` fixed it.
- Hiding the collection view behind the empty state removed the large title in the iOS 26 navigation bar, so the
  collection view now stays visible (empty).

## Guesses
- Quantities in the data are whole numbers 1–4 with odd units ("2 cloves eggs"). I scale the number only and don't
  change units or plurals.
- I used a chip row for the category filter rather than a menu, so it's visible and quick to change.

## Not done / would have helped
- `simctl` has no way to tap or type, so I drove interactive paths (stepper taps, swipes, typing in search) through
  `-debug…` launch arguments (`DebugLaunch` in `AppDelegate.swift`) instead of real touches. A tap/type tool, or XCUITest,
  would have made testing faster and more faithful.
- No app icon (the skeleton has no asset catalog step).
