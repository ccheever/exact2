# Expenses build notes

Finished the programmatic UIKit app with bundle ID `dev.bakeoff.expenses.ba282f1`.
It is built, installed, and left running on simulator
`CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was booted, erased,
or used. The final build completes without compiler warnings.

## Implemented

- All 240 supplied transactions ship unchanged as `Resources/transactions.json`.
- Newest-first activity, day grouping and day totals, monthly summary, seven
  categories sorted by spending, percentage shares, and monthly category drilldown.
- A native add/edit sheet with amount validation, merchant, category menu, date
  picker, optional note, keyboard Next/Done controls, and unsaved-change protection.
- Swipe deletion and immediate atomic local JSON persistence. Empty saved history
  stays empty rather than being reseeded. Totals update across both tabs.
- Light/dark appearance, Dynamic Type through the largest accessibility size,
  multiline merchants, VoiceOver labels, native navigation, and a custom icon.

## Hard parts and decisions

- Currency is stored as integer cents and decoded/encoded through Decimal to avoid
  floating-point rounding errors. Amounts must be positive, use at most two decimal
  places, and be at most $999,999,999.99. Both period and the current locale's decimal
  separator are accepted. Merchant is required; whitespace-only merchants are rejected.
- Dates are local Gregorian calendar dates rather than UTC instants. This preserves
  the supplied date-only data across daylight-saving and timezone boundaries.
  “This month” means the actual current local calendar month, not the seed's month.
- Category detail shows the current month's transactions because it drills into the
  current-month category totals. Zero-spending categories remain visible with 0%.
- Large text needed vertical row layouts and a wrapping breakdown button. Simulator
  review caught and fixed clipped button text, currency alignment, and navigation
  title behavior. The category detail uses a compact native navigation title.
- The starter's README and `simctl` command help supplied the toolkit details. No
  web research, third-party libraries, or downloaded assets were needed.

## Verification

The opt-in simulator runner exercises the real form save methods and swipe-delete
handler, not a separate implementation. All 24 checks passed on the final build:
seed integrity, seven categories, exact totals, amount edge cases, date boundaries,
grouping, default date, form validity, create/edit/delete, persisted reloads,
category sorting/drilldown, and intentionally empty history. After those operations
and relaunch, an external comparison confirmed that saved history matched all 240
original records exactly. No test expenses remain.

Reviewed simulator screenshots of activity, categories, add/edit forms, category
transactions, and scrolled content in dark mode at the largest accessibility text
size. Layout log checks found no constraint conflicts. The simulator check toolkit
has no touch injection, so these are handler-level functional checks and screenshot
reviews, not complete end-to-end gesture automation. A software-keyboard screenshot
was not obtained; the standard UIKit decimal keyboard, focus handling, accessory
controls, and keyboard-aware table controller are implemented.

Reproduction instructions are in `Tests/README.md`; the final check report and
screenshots are in `build/`. No known required app features are unfinished.

## What would have made this faster

A ready UIKit scene-delegate skeleton, a warm compiler/simulator, and supported
simulator touch/text injection plus accessibility-tree inspection would shorten
setup and allow fully automated end-to-end keyboard, menu, swipe, and dismissal
checks. Screenshot inspection was valuable but slower than direct UI assertions.
