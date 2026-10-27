# Recipes (Exact): notes

Built from Exact's `tabs` pattern. `app.contract` holds the view and logic, and `app.ts` loads
`data/recipes.json`, which is bundled at build time by a JSON import. `app.test.contract` has
7 tests. On the final runs, `verify ios` came back clean: tests, light and dark, the largest
text size and real-touch scrolling. The app is installed on simulator
C2188CC6-FBE9-409A-92A8-ABD89AC2C432 as `dev.bakeoff.recipes.b50a521` ("Recipes").

## What's there
- **Recipes tab:** a large title, a search field in the bar that matches the title or an
  ingredient name, but not the quantities, so "2" doesn't match every recipe. A Category
  pull-down (All plus the nine categories) and a Sort pull-down (Name; Time, quickest first;
  Rating, highest first) both use native UIMenus with checkmarks. The section header says
  what is shown, e.g. "Desserts · 28 Recipes · by Name". Each row shows the title, category,
  time ("1 hr 5 min") and a star rating. At the largest text sizes the rating wraps under
  the title instead of crushing it. Changing the category or sort scrolls back to the top.
- **Empty results:** these name the category and the query. A search with no match inside
  one category offers "Search All Categories", placed above the keyboard.
- **Recipe screen:** the title, with the category as its subtitle, and a heart to add or
  remove a favorite. A summary card shows time, serves and rating. Under Ingredients, a
  UIStepper servings field (1–99) scales every amount, shown as cooks write them (1½, ¾, ⅓;
  decimals only when no fraction fits). A Reset button appears once scaled, and the footer
  says "Scaled from 4 servings to 5." Steps are numbered.
- **Favorites tab:** search, and swipe-to-delete on rows. When empty, it says "No Favorites
  Yet", explains how to add one, and offers a "Browse Recipes" button. Favorites are a
  `persist` state, which is UserDefaults on iOS; I checked by relaunching on the same store,
  and a test does too.

## What was hard / what I guessed
- **No `let` inside a view:** computing the scaled servings once per recipe isn't possible
  there, so it's a small `fn` called in each place.
- **Formatting fractions without loops or recursion:** I rounded to 24ths and looked up a
  glyph table with `slice`.
- **Rows read like Settings rows:** a row's second `text` is right-aligned as a trailing
  value, so ingredients and steps first rendered right-aligned. Wrapping the texts in an
  inner `row flex=1` fixed it.
- **Scroll reset:** re-keying the list (`for v in [key] key=v`) to reset scroll on a filter
  change is my own trick; I found no documented way to scroll to the top.
- **Sizing:** fixes found by looking at the largest text size: the summary card wraps
  (`flex-wrap` plus `flex-basis` in rem), and the empty Favorites screen scrolls with a
  shorter message.
- **Icons:** `sf/clock` would be blank on the web, so I added `app.tamber` with an icon token
  and a hand-drawn `assets/icons/clock.svg`. Linux shipping was removed
  (`update --without linux`).
- **Spec reading:** I assumed "servings" in the spec means the recipe's own count (shown in
  the summary card), with the stepper as a separate scaling control. The ingredient amounts
  in the data are all whole numbers 1–4 with no fractions, but the parser accepts decimals
  and `a/b`.

## Not finished / known leftovers
- The web `verify` still reports 16 findings, none of them on iOS:
  - 14 are the Category menu while it's open, painted in the page on the web, overlapping
    the content. On iOS it's a native UIMenu.
  - 2 are a horizontal page offset on the web detail screen at 53 px root text, which I
    didn't track down.

## What would have made it faster
- A documented way to reset a list's scroll position, and a way to write a leading
  (left-aligned) secondary text in a row without the wrapper trick.
- `let` bindings in view loops.
- `verify` (web) separating "a menu is open" overlaps from real layout overlaps.
