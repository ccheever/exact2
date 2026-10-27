# Recipes (UIKit) — notes

## What's there
- **Recipes tab**: compositional-layout collection view. A horizontally scrolling row of category chips (All plus the
  nine categories) sits above a plain list. Each row shows the title, category, time and rating. The search controller
  matches titles and ingredient names; multiple words must all match, and matching ignores case and diacritics. When
  a hit comes from an ingredient, the row says "Contains …". Category filter and search combine. The Sort menu offers
  Name / Time (quickest first) / Rating (highest first), ties broken by name, and the choice is persisted. The nav
  subtitle shows the live count ("11 Pasta recipes"). The no-results state explains which filter is excluding
  everything and has a "Search All Categories" button.
- **Detail**: big wrapping title header with Time / Servings / Rating cards. Below it a servings stepper (1–24) with
  ingredients scaled live. Quantities show as readable fractions (¾, 1½, ⅓…), units are pluralised (1 cup / 2 cups,
  1 clove), and a footer notes the original yield. Steps are numbered. A heart in the nav bar adds/removes the
  favorite. Long-press on the servings row resets it.
- **Favorites tab**: the same list with its own search and sort (sorted by name by default). Swipe to remove. The empty
  state ("No Favorites Yet" + how to add + "Browse Recipes" button) hides the search bar. Recipe rows in the main list
  also support swipe-to-favorite, a context menu with a preview, and a heart badge for saved recipes.
- **Persistence**: favorites are stored as JSON in Application Support, written atomically on every change.
  Verified across relaunches.
- Dark mode, and AX5 text: the stat cards turn into compact rows, the stepper moves under its label, and list
  metadata wraps only between facts (non-breaking spaces). VoiceOver labels on rows, stats, steps and the stepper,
  plus custom actions for favoriting.

## What was hard / guessed
- The first launch crashed: a diffable cell registration typed with `String?` fails a dynamic cast inside UIKit
  when the item is `nil`. I fixed it by registering with the `Item` enum.
- The data's ingredients look like "2 cloves eggs" / "1 stock" (generated). I guessed a unit list (cup, tsp, tbsp,
  g, cloves, …) so the "Contains …" text and search work on the food, not "g eggs". Quantities in the data are
  all integers; the parser also accepts decimals and a/b.
- Scaling rounds to the nearest common vulgar fraction within 0.005 and otherwise shows a decimal.
- No UI automation tool was available, so I added `-Demo…` launch arguments (category, search, sort, open recipe,
  servings, scroll, tab, favorite) to drive the app via `simctl launch` for screenshots. They do nothing in a normal
  launch. `shot.sh` wraps them.
- Swipe actions, context menus, and the toggle/remove animations could not be exercised by touch. They are verified by
  code path and by launch-argument-driven state only.

## What would have made it faster
- A way to tap/scroll in the simulator from the CLI (idb or similar), or XCUITest support in the build script.
- Crash output on the console when `build.sh --run` launches (I had to dig the .ips report out of DiagnosticReports).
