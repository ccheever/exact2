# Expenses — build notes

Finished the programmatic UIKit app, built it with `build.sh`, and left it installed and open on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`.

- Name: Expenses
- Bundle ID: `dev.bakeoff.expenses.b74b921`
- All 240 original transactions are bundled in `Resources/transactions.json` and remain in the installed app. Test expenses were removed.
- Additions, edits, and swipe deletions save atomically to Application Support. An empty saved history is preserved rather than seeded again.
- No specification features remain unfinished.

## Decisions

“This month” uses the current local Gregorian calendar month. Category detail lists use the same month as the category overview. Dates are stored as calendar-day strings to avoid timezone shifts. Money uses Decimal and integer cents for totals, with US currency formatting. Amounts must be positive and have at most two decimal places; merchants are required and notes are optional.

## What was hard

The most involved work was self-sizing table headers and forms at the largest accessibility text size. Screenshot review caught a weighted-font helper applying Dynamic Type scaling twice. The corrected rows stack their details at accessibility sizes, keep currency amounts together, and scroll long notes to keep the cursor above the keyboard.

The native compact date picker exposes a larger accessibility frame than its visible button. Sizing the control and using precise calendar gestures made both real interaction and simulator tests reliable. A first test gesture selected the wrong September day; the corrected test verifies September 28 before saving and again after relaunch. Unsaved-change cancellation was also checked through both Keep Editing and Discard Changes.

## Lookups and verification

Only local tool help was needed for simulator appearance/text-size controls and exporting XCTest screenshots. No web research or external dependencies were needed. A small standalone UI-test project was added because the starter did not include a test harness.

Passed model checks: all 240 unique IDs, exact seeded month/day totals, invalid and valid amount parsing, US currency output, leap-day/date round-trips, and JSON encoding/decoding.

Passed final simulator checks: category navigation, empty/zero amount validation, required merchant validation, adding all fields, editing amount/merchant/category, swipe deletion, persistence after termination and relaunch, exact date changes across month boundaries, unsaved-change handling, dark mode, the largest accessibility text size, and long-note keyboard entry. October's starting total is $5,269.70 across 86 expenses. Simulator appearance and text size were restored to light/normal afterward.

Test sources, final successful result bundles, logs, and reviewed screenshots are under `QA/`; `QA/README.md` explains reproduction. The built app is `build/App.app`.

## What would have made this faster

A starter UI-test target, documented compact-date-picker automation, and a ready-made Dynamic Type screenshot matrix would have reduced setup and iteration. An initial visual reference would also have reduced the amount of design judgment needed.
