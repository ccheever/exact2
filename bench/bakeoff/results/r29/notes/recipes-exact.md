# Recipes (Exact) — notes

Built from Exact's `tabs` pattern. Bundle id `dev.bakeoff.recipes.bb499ee`, name `Recipes`. The app
is installed on simulator C2188CC6-FBE9-409A-92A8-ABD89AC2C432. All 5 tests pass on the web and on
iOS. `verify ios` came back clean: tests, light and dark, the largest text size, scrolling and
relaunch. Web `verify` still lists some largest-text-size findings, covered below.

## What's in it
- **Recipes tab:** all 160 recipes, each row showing title, "category · time" and a ★ rating. The
  bar has a search field that matches the title or an ingredient name, a **Category** pull-down
  (All plus the nine categories, with a checkmark on the current one) and a **Sort** pull-down
  (Name A–Z, Time quickest first, Rating highest first; ties fall back to name). Search and
  category work together. A section header shows the current state ("Soups · 5 recipes · by name").
  When nothing matches, an empty state offers "Show All Recipes", which clears the search and the
  category. The sort order is kept across launches.
- **Recipe screen:** title, category and rating, then time and servings. Servings uses UIKit's
  stepper (1–99), and you can also type a number into it. Ingredients are scaled and shown as
  fractions (½, ⅓, ¾ …). The header says "Ingredients for N", and "Recipe makes N" appears when the
  servings differ from the original. Steps are numbered. A heart in the bar adds the recipe to
  Favorites or removes it.
- **Favorites tab:** saved recipes, which you can search and swipe to remove. When it's empty it
  shows "No Favorites Yet / Open a recipe and tap the heart…" with a "Browse Recipes" button.
  Favorites are a `persist` state; I checked that they survive a real terminate and relaunch on
  the simulator.
- `app.ts` imports `data/recipes.json`, so the data ships with the app. It splits each ingredient
  line into quantity, unit and name once at load.

## What was hard / guessed
- **Ingredient format:** I checked the data. Every quantity is a whole number from 1 to 4, and the
  units are cup, tsp, tbsp, g and cloves, or none. Units are shown as the data has them ("4 cup
  mushrooms") and not pluralised. Scaled amounts are rounded to the nearest ⅛, ¼, ⅓, ½, ⅔, ¾ or ⅞;
  at 20 or more they are rounded to a whole number.
- **The stepper's scale is kept for one recipe at a time** (the last one you changed). Opening a
  different recipe shows that recipe's own servings.
- **Name clashes:** Contract refused actions named `filter` and `sortBy` because they shadow
  built-ins. The compiler's messages made this easy to fix.
- **Rows inside a list:** two `text`s in a row are laid out as a label and a right-aligned value.
  To get "amount + ingredient" side by side I wrapped them in an inner `row`.
- **Row height:** the list adds a 15 pt margin to a column inside a row, which made rows tall. I
  set `margin-top=0 margin-bottom=0`, which brought them down to about 67 pt. I found the cause
  with `agent … layout`.
- **Web screenshots:** the agent captures pull-down menus as "painted" text without checkmarks. A
  drive with `--touch platform` showed the real UIMenu, checkmarks included.

## Not done / known
- **Web-only findings at the largest text size:** web `verify` still lists 14 overlaps from the
  web-drawn menu while it's open mid-test, and one overflow during a push transition. iOS has
  neither: `verify ios` is clean.

## What would have made it faster
- A list-row part with a title, subtitle and trailing value (like UIKit's subtitle cell), so I
  wouldn't have had to work out the default margins.
- A note in the `menu` docs that the agent's normal screenshots draw native menus as painted text.
