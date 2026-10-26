# Expenses — build notes

The finished app is built and installed as **Expenses**, bundle ID `dev.bakeoff.expenses.b577f3e`, on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used. The final install was `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios`.

## Delivered

- All 240 original transactions ship through the data module's JSON import. The original `data/transactions.json` is unchanged.
- Newest-first history, grouped by calendar day with exact day totals, plus the current month's total.
- Native add/edit sheets with amount validation, all seven categories, calendar date selection, and optional multiline notes.
- Native swipe-to-delete; atomic file persistence for adds, edits and deletes.
- Category totals, descending order, percentage shares, and this month's transactions for each category.
- Native navigation, tabs, keyboard controls, dark mode, Dynamic Type, and a custom app icon.

## Verification

- `bun exact.mjs verify`: **7 passed, 0 failed**, no compiler warnings, clean light/dark and largest-text checks, and successful relaunch checks.
- `bun exact.mjs verify ios`: **6 passed, 0 failed**, no light/dark layout findings, no overflow at accessibility extra-extra-extra-large, successful persistent-data relaunch.
- The additional category-move test passed separately on iOS with `bun exact.mjs test ios category.test.contract`.
- Real simulator touches checked category navigation, opening and reopening edit sheets, and revealing the native Delete swipe action. Native functional tests verified deletion and its persistence.
- Tests cover the seed totals, ordering, all fields, malformed/negative/zero amounts, one-cent amounts, editing into another month, deleting, canceling changes, empty months, local midnight, category changes, and relaunching.

## What was hard and what I looked up

Exact's dashboard starter and recipes were useful, especially grouped forms, money, dates, tabs, edit sheets, persistence, and iOS driving. Currency is stored as integer cents; the view supplies the device's local calendar date because the data module intentionally has no clock.

The main native issue was reproducible with real touches: after an edit sheet dismissed, the grouped list remained visible but stopped accepting row taps. Flattening the row/link structure and changing individual row keys did not fix it. The app now recreates the list host after dismissal and restores the edited transaction into view. Reopening and saving then passed on iOS. Restoration follows the edited item rather than preserving the exact previous pixel offset; if it leaves the category, the category list returns to its beginning.

Large text required flexible headings, appropriate tab captions, and Exact's fitting numeric readout. Moving navigation properties onto the form component's root was necessary to get a true native sheet navigation bar. An invalid tint string inside a component prop compiled but produced empty category bars; a documented system color fixed it.

## Decisions

“This month” follows the device's local calendar month. Category details show the same month's transactions as the breakdown. Future dates are allowed. Amounts are rounded to cents and must remain positive; shares are displayed to one decimal place. Missing saved storage uses the bundled seed, while a deliberately empty saved history remains empty.

## Remaining limitation

No requested functional feature is missing. **UIKit-equivalent scrolling performance was not achieved.** Exact's native verifier still reported seven scrolling findings on this machine: a 50.7 ms/s hitch ratio, an 80.9 ms longest frame, 11.01 ms/frame main-thread p95, tracking ratio 0.926, three frames of touch lag, deceleration discontinuities, and slightly slower status-bar scrolling to the top. Rows contain only text, without images, shadows, blur, or per-frame app scroll handlers. The verifier identifies this situation as host overhead; simplifying the row structure did not eliminate it. Full measurements are in `.exact/verify/ios-feel/feel.json` and `feel.txt`. The native verification therefore passes its functional and appearance checks but is not entirely clean.

## What would make this faster

A toolkit fix for grouped-list interaction after sheets, lower native list rendering overhead, validation of color-valued component props, and a seeded-history starter combining tabs, grouped dates, and editable expense sheets would save the most time. The focused vocabulary command and automatic accessibility/layout checks were particularly helpful.
