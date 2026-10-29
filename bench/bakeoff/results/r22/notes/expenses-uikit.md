# Expenses — build notes

Finished, built, installed, and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86` as `dev.bakeoff.expenses.b14480b`.

## What ships

- A programmatic UIKit app delegate, scene delegate, navigation controllers, and two tabs.
- All 240 supplied transactions bundled in `Resources/transactions.json`. The history seeds once and is then saved atomically in Application Support. An intentionally empty saved history stays empty.
- Day-grouped, newest-first transactions with daily totals and this month’s summary; add, edit, swipe delete; seven-category monthly breakdown, percentage shares, and category drill-down.
- Exact integer-cent arithmetic, US currency formatting, positive-amount validation, merchant validation, native date selection, optional multiline notes, keyboard dismissal, unsaved-change protection, dark colors, and Dynamic Type layouts.
- A custom ledger app icon, compiled with `actool` by `build.sh`.

## Decisions

“This month” uses the device’s current local calendar month. Category drill-down shows that month’s transactions so it agrees with the breakdown. All seven categories remain visible even with zero spending. New entries default to Dining, or the category currently being viewed. Entries on the same day use descending ID as a stable tie-breaker because the source has no times. Amounts accept up to two decimal places and reject zero, negative values, invalid text, and excessively large input.

## What was hard, and what I looked up

The skeleton had no Xcode project or UI-driving tool. I created a separate XCUITest harness while retaining the requested direct `swiftc` production build. UIKit’s form sheet exposes underlying history cells to UI tests, so the category test needed to count only category-picker identifiers. The compact date picker exposes a button rather than a DatePicker element; the tests now use its containing cell.

At maximum accessibility text size, screenshot review caught weighted labels scaling twice. Font construction now uses an unscaled base font, with a single Dynamic Type scaling pass. Accessibility layouts stack amounts beneath merchant/category names. The summary currency fits on one line, and the Save button keeps white-on-dark-green contrast in dark mode.

I consulted local `simctl`, `xcodebuild`, and `xcresulttool` help for appearance/text-size controls, test diagnostics, and screenshot export. No web research or extra dependencies were needed. Automatic failure diagnostics were slow; subsequent test runs disabled them.

## Verification

- Model checks passed for the 240-record seed, seven categories, October’s exact $5,269.70 total, valid/invalid amounts, add/edit/delete, persisted notes and amounts, month-boundary edits, relaunch, and an empty saved history.
- UI lifecycle, discard protection, and navigation tests passed. The lifecycle test added an expense, relaunched, verified saved fields, edited the amount, swiped to delete, and relaunched again.
- Maximum-accessibility-size dark-mode tests passed for expense entry, reopening, and deletion. Appearance screenshots were reviewed for the history, category breakdown, category transactions, and form. Native date selection and restoring an edited date passed.
- No Expenses Auto Layout constraint warnings were found in the simulator log.
- Final `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run` completed without compiler warnings. The final bundle includes the seed and compiled icon assets. Temporary test entries were deleted and the edited seed date restored. The UI test runner was removed; the app remains installed, with standard text size and light appearance restored.

Tests and their instructions are in `Tests/`. Logs, result bundles, and screenshots are in the ignored `build/` directory.

## Unfinished work

None of the requested features remain unfinished. Storage is local to the app; no account or cloud service is required.

## What would have made this faster

A ready-to-run UIKit skeleton with a scene manifest, asset catalog compilation, and a small simulator UI test target would have avoided most setup and test-harness work.
