# Recipes (UIKit): notes

## What's there
- **Recipes tab:** a plain table with a large title, a search bar that stays visible, and a sticky row of
  category chips (All plus the 9 categories) as the section header. A pull-down sort menu in the
  navigation bar offers Name (A–Z), Time (quickest first) and Rating (highest first); ties are broken by
  name, and the chosen sort is remembered. Search matches titles and ingredient names
  (case- and diacritic-insensitive), works together with the category filter, and labels rows that
  matched on an ingredient with "Contains …". When nothing matches, an empty state names the
  category and query and offers "Search All Categories".
- **Rows** show a category icon, title, category, time, a star rating and a heart if saved.
  Swipe right to add or remove a favorite; long-press opens a context menu with a preview.
- **Detail screen:** category, big title, cards for time, servings and rating, a servings stepper
  (1–50), ingredients and numbered steps. Quantities scale and are shown as kitchen fractions
  (1½, ⅔, ¾). The ingredients header reads "Ingredients for N servings" when scaled. The title moves
  into the navigation bar once you scroll past it, and a heart button saves or removes the recipe.
- **Favorites tab:** most recently saved first, with swipe-to-remove and Edit. The empty state explains
  how to save a recipe and has a "Browse Recipes" button. Favorites are stored in UserDefaults and
  survive relaunch; I checked this by saving in one launch and seeing them after the next.
- Built with dynamic type throughout. At accessibility text sizes the stat cards and the stepper row
  stack vertically. Uses only system colors, so dark mode works. VoiceOver labels are set on rows,
  stats, steps, the stepper and the heart button.

## What was hard / guessed
- The category chips collapsed into vertical text the first time the header re-laid out (while
  search was active). The fix was making the button titles single-line (`titleLineBreakMode =
  .byClipping`) with required compression resistance.
- `contentUnavailableConfiguration` covers the whole table, including the sticky chip bar, so the
  list's empty state uses `tableView.backgroundView = UIContentUnavailableView(...)` instead.
- Inset-grouped tables add a tall default header above the first section; I removed it with a
  near-zero-height `tableHeaderView`.
- Guesses: the data has no fractional quantities and a few odd lines ("2 cloves eggs", "1 stock").
  I scale the leading number, keep the unit word and display everything else as given. The parser
  also accepts decimals and "a/b". Scaled amounts round to the nearest common fraction and never
  round down to zero.
- Category icons and colors are my own choices (SF Symbols, with a fallback if one is missing).

## Testing
- `simctl` can't tap, so I added a temporary launch-argument hook to open screens, set searches,
  filters, sorts and servings, and seed favorites. I took screenshots in light, dark and
  accessibility XXXL text, then removed the hook. I never actually tapped the stepper, swipe
  actions or menus; those use standard UIKit behaviour.

## What would have made it faster
- A way to tap and type on the simulator (for example idb or XCUITest), so interactions can be
  tested directly instead of through launch-argument hooks.
