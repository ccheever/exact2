# Recipes (UIKit) — notes

## What's built
- UIKit, programmatic, scene-based (`Sources/AppDelegate.swift`; the scene manifest is in `EXTRA_PLIST`). `data/recipes.json` is copied into `Resources/` so it ships at the root of the bundle.
- **Recipes tab**: a compositional-layout collection view. It has a horizontally scrolling row of category chips (All plus the 9 categories) and a list of recipes with a category icon, title, "category · time" and a ★ rating badge. A search controller searches titles and ingredient names. Each word you type must match the title or an ingredient; matching ignores case and accents. When a match comes only from an ingredient, the row says "With garlic". The category filter and search work together. The Sort menu (Name A–Z, Time quickest first, Rating highest first; ties are broken by name) is remembered between launches. A header shows the result count and the sort order. When nothing matches, a "No Recipes Found" message appears; inside a category it adds a "Search All Categories" button. You can swipe a row or long-press it to favorite or unfavorite.
- **Recipe screen**: a summary card (category, a title that wraps, time, servings and rating), a servings stepper (1–24) with Reset, ingredients, and numbered steps. Changing servings rescales the quantities into kitchen fractions (1½, ⅔, ⅞…). If a value isn't close to a fraction, it falls back to a short decimal; grams always use decimals. Units switch between singular and plural (cup/cups, clove/cloves). The heart button in the nav bar saves or removes the recipe. The nav-bar title fades in once the summary card's title scrolls away.
- **Favorites tab**: newest first. Swipe or long-press to remove. When empty it shows "No Favorites Yet", explains how to add one and has a "Browse Recipes" button. Favorites are saved as ids in `UserDefaults`, so they survive relaunch; I checked this by relaunching the app.
- Dark mode uses system colours throughout. At the accessibility text sizes, the list rows move the rating onto its own line and drop the icon, and the summary stats and the servings row stack vertically. VoiceOver labels, values and custom actions are set.

## What was hard / guessed
- The simulator has no tap tool, so I added test-only launch arguments to drive state: `-debugCategory`, `-debugQuery`, `-debugSort`, `-debugOpen <id>`, `-debugServings`, `-debugScroll <section>`, `-debugFavorites "1,2"` and `-debugTab 1`. They do nothing unless passed. I checked each screen with `simctl io screenshot`. Real touch gestures were not driven: swipe actions, context menus, the stepper and tapping the chips. Those are standard UIKit APIs and the code paths they call were exercised through the hooks.
- Crash: a `CellRegistration` whose item type is `String?` crashes on a nil item (NSNull cast). I fixed it by registering with the enum item instead.
- The data is synthetic. Lines like "2 cloves eggs" and unitless "1 stock" are shown as given. I guessed that a quantity is the leading number and that the unit is one of cup/tsp/tbsp/g/clove(s).
- I picked the category icons and colours myself (SF Symbols, falling back to `fork.knife`).

## Not done
- No real UI tests (XCTest isn't available in this setup). There is no app icon.

## What would have made it faster
- A way to send taps and swipes to the simulator, or an accessibility inspector, from the command line.
