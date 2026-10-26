# Recipes (UIKit) — notes

## What's there
- Tab bar: **Recipes** and **Favorites**, each a navigation stack with large titles. Built in code: app delegate + scene delegate (the scene manifest is in `EXTRA_PLIST`).
- **Recipes list**: rows show the title, category, time and rating, with a heart on favorites. The subtitle gives the result count. There's a search bar (titles *or* ingredient names; every word must match; case- and diacritic-insensitive), a row of category chips (All plus the nine categories), and a sort menu (Name A–Z, Time quickest first, Rating highest first; ties go by name; the choice is remembered). Search and category filter work together. Rows that matched only on an ingredient say "Contains mushrooms". The empty state names the query and offers "Search All Categories" when a category filter is on.
- **Recipe detail**: category, title, and boxes for time, servings and rating. A servings stepper (1–99) scales the ingredient amounts, using kitchen fractions (½, ⅓, ¾ …) for counts, cups and spoons, and decimals for grams. A footer shows the original serving count. Steps are numbered. The heart in the nav bar adds or removes the favorite.
- **Favorites**: newest saved first. Swipe or Edit to remove, drag to reorder. The empty state explains how to save one and has a "Browse Recipes" button. Saved in `UserDefaults` (ids it doesn't recognise are dropped on load), so favorites survive relaunch (checked by relaunching).
- On rows: swipe right to add or remove a favorite, long-press for a preview plus a favorite action. Works in dark mode and at AX5 text size (detail stats switch to a stacked layout; times use non-breaking spaces).

## What was hard / guessed
- **Driving the UI without taps.** `simctl` can't tap, so I added test-only launch arguments (`-preset-query`, `-preset-category`, `-preset-tab`, `-preset-open <id>`, `-preset-servings`, `-preset-steps`, `-preset-favorite`). They let me screenshot each state. They do nothing unless passed. The scaling math was checked by compiling `Model.swift` into a macOS test binary.
- **Header and empty-state layout.** Sizing the `tableHeaderView` chip bar by hand, and getting the selected chip to scroll into view, needed the scroll deferred until after layout. The stat boxes clipped text at AX sizes until `fillEqually` was used only in the horizontal layout.
- **Guesses:**
  - Ingredient format is "qty [unit] name". The units are cup/tsp/tbsp/g/cloves.
  - Units aren't pluralized, to match the data ("4 cup mushrooms").
  - Grams scale as decimals.
  - The smallest amount shown is ⅛.
  - Search matches ingredient *names*, not quantities.

## Not done
- No tap-driven UI tests (no XCTest or tap tool available). Swipe, context-menu, stepper-tap and sort-menu interactions were checked by code review and the preset hooks, not by tapping.

## What would have made this faster
- A way to send taps and scrolls to the simulator (or `simctl` accessibility queries).
