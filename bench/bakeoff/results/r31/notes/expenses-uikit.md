# Expenses — build notes

Finished as a programmatic UIKit app. The final build is installed and running on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`, with bundle identifier `dev.bakeoff.expenses.b30aedf` and display name **Expenses**.

## Implemented

- All 240 supplied transactions are bundled in `Resources/transactions.json` and imported exactly once.
- Newest-first history grouped by date, with day totals and the current month’s total.
- Add and edit amount, merchant, one of the seven categories, date, and optional multiline note. Swipe left to delete.
- Monthly category totals, descending order, percentages, and transaction drill-downs.
- Exact integer-cent arithmetic, US currency formatting, atomic JSON persistence, and an empty-history state that remains empty after relaunch.
- Native navigation, modal forms, date picker, decimal keyboard with Done, protection against discarding unsaved changes, dark mode, VoiceOver labels, and Dynamic Type layouts through the largest accessibility size.
- A custom receipt app icon.

## Verification

Built using `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`. Only that simulator was used.

The installed app passes 27 isolated simulator checks, recorded in `artifacts/smoke-test.txt`. These cover seed integrity, the $5,269.70 October 2026 total, amount boundaries, merchant validation, Save and swipe Delete action handlers, category selection, edits across dates/categories, persistence, sorting, monthly filtering, and deleting the last expense without reseeding. Tests use separate files and leave the user history unchanged; a final comparison confirmed the installed history exactly matches the 240 original records.

Reviewed simulator screenshots in light mode, dark mode, with the decimal keyboard, and at the maximum accessibility text size, including transaction rows, category rows, the category picker, and the editor’s date field. Screenshots are in `artifacts/`. The verification uses in-app UIKit action checks and `simctl` screenshots rather than an XCTest gesture runner.

To rerun the isolated checks after building, terminate the app and launch it with `--smoke-test` using `xcrun simctl`; the report is written to the app’s Documents directory. Launch normally afterward.

## What was hard

Self-sizing table headers and rows needed careful layout work at the largest text size. Screenshot testing caught a double-scaling error in weighted fonts; the corrected helper starts from the default-size font and scales once. At accessibility sizes, transaction/category content stacks vertically so amounts and notes remain readable.

The initial test harness caused unrelated controllers to reload on every test mutation. Scoping change notifications to their ledger and using a small separate fixture for the last-expense test made verification fast and kept production history isolated.

## Choices and lookups

No external research or dependencies were needed. Compiler diagnostics identified an iOS 26 bar-button style rename and a selector name conflict; both were resolved.

“Month” means the device’s current local Gregorian calendar month. Category drill-downs show that month’s expenses, matching the category totals. The default category for a new expense is Dining, or the category currently being viewed. Merchant is required. Amounts accept a dot or comma decimal separator, permit at most two fractional digits, and range from $0.01 to $999,999,999.99. Dates may be in the past or future because the specification does not restrict them.

## Unfinished work and faster tooling

No known unfinished specification features. A ready-to-use simulator gesture/accessibility test driver and a standard UIKit sizing/test scaffold would have shortened the visual verification loop. The minimal build script worked well and required no Xcode project or third-party packages.
