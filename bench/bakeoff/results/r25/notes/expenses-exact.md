# Expenses — build notes

Built and installed as **Expenses** (`dev.bakeoff.expenses.b17aec8`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final packaged app was launched with `bun exact.mjs ios --run`.

## What shipped

- All 240 transactions from `data/transactions.json`, bundled through the data module as the starting history.
- Transactions sorted newest first, grouped by date with accurate daily totals.
- Current local calendar month's spending summary.
- Native add/edit sheets with amount, merchant, seven-category picker, date picker, and optional note.
- Positive amount validation, integer-cent calculations, and US currency formatting.
- Native swipe deletion, including a real-touch full-swipe check.
- Monthly category totals, largest first, percentage shares, and dated category histories.
- Atomic local storage for additions, edits, and deletions; relaunch persistence tested.

## What was hard

The most significant issue was native sheet dismissal while refreshing the underlying collection. A mutation declared with `refreshes ledger then afterSave` could leave the history visually blank after saving, despite correct stored data. Closing the sheet in the successful continuation and then explicitly refreshing the ledger fixed it. Both real-touch drives and the complete native test suite confirmed the fix.

The first Exact compiler and iOS runtime builds took several minutes. Subsequent plan updates and the final packaged build were fast. Some initial toolkit compilation required sandbox escalation because its compiler output lives in the provided Exact checkout.

## What I looked up or decided

I used Exact's dashboard starter, then consulted its tabs, edit-sheet, grouped-form, swipe, money, dates, file-storage, navigation, numeric-readout, and simulator-driving references.

“This month” means the device's current local calendar month. Category detail lists that month's transactions, matching the category summary. Same-day transactions use descending numeric IDs as a deterministic tie breaker because the input contains dates rather than timestamps. A new expense defaults to Dining and today's local date. Entered amounts round to cents and are limited to $999,999,999.99 to keep the input and arithmetic bounded.

The original history is kept intact in the source data. Saved history is a separate local JSON file; an empty saved history remains empty after relaunch rather than restoring the seed.

## Verification and remaining limitations

`bun exact.mjs verify ios` finished **clean**:

- 8 tests passed, 0 failed.
- Light and dark appearances had no layout findings.
- Largest accessibility text size (53-point body), across 10 screens, had no overflow.
- Real-touch scrolling had nothing for the app to fix. Absolute timing judgment was skipped because the shared machine was busy; the probe also reported a small Exact-host status-bar-scroll timing difference.
- Relaunch showed the persisted edited/deleted history correctly.

Additional real-touch drives checked add/save, date-changing edits, opening another transaction, cancellation, and native swipe deletion. The data module also passed direct checks for seed totals, invalid amounts and dates, adding, and deleting. The independently calculated October seed total is $5,269.70.

No requested iPhone feature remains unfinished. Web verification passed all 8 functional tests and ordinary-size appearance checks, but its largest-text approximation retained 3 findings involving a navigation heading and partially visible scrolled form text. Native iPhone verification was clean.

## What would have made this faster

A bundled-history expense pattern with tabs and editing would reduce setup. A documented native-safe refresh/dismissal sequence, stronger diagnostics for that ordering issue, and clearer progress during cold native/tooling builds would have saved the most time. A prebuilt real-touch harness would also shorten the first simulator verification.
