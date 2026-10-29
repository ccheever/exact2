# Expenses build notes

Completed and installed as **Expenses**, bundle ID `dev.bakeoff.expenses.e68e11`, on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final installation was launched with `bun exact.mjs ios --run` and `EXACT_SIM` set to that simulator.

## Implementation

- All 240 original transactions from `data/transactions.json` are embedded in the data module as starting history. A separate atomic JSON file stores subsequent additions, edits and deletions, including an empty history.
- Money is converted to integer cents at the data boundary. Transactions sort by descending date, then descending ID within a day. Daily and monthly totals derive from the same records.
- Native grouped lists, tab navigation, modal forms, category menus, date pickers, and swipe deletion provide the iPhone interface. Amounts use USD formatting. Open form drafts also survive reloads.
- Categories show the current calendar month, sorted by spending. I interpreted category drilldowns as that category’s transactions for the displayed month. All seven categories remain available when their spending is zero; zero-total shares display 0%.
- Dates are stored as calendar dates without timezone conversion; new entries default to the current local date. Future expense dates are allowed because the specification does not prohibit them.

## Validation

`bun exact.mjs verify` and `bun exact.mjs verify ios` both finished **clean**. All eight behavioral tests passed on both hosts. The checks covered light and dark modes, the largest accessibility text size, saved data after relaunch, exact starting totals, positive-amount validation, add/edit fields, cancellation, date/month changes, category ordering and shares, deletion, empty months, and local-midnight date defaults.

A separate real-touch simulator drive confirmed that a full native swipe deletes a transaction: the isolated test history changed from 240 to 239 records. Category screens also passed light/dark checks. Test writes used isolated stores, leaving the normal app’s starting history intact.

## What was hard / what I looked up

- The first Exact compiler and native builds needed toolkit/cache writes outside the app sandbox. Later iOS plan updates were much faster.
- I used Exact’s guide, dashboard/tabs patterns and vocabulary for section headers, date inputs, money, persistence, modal forms, symbol roles and swipe actions.
- List-row component wrappers can interfere with native cell boundaries. Transaction loops are written directly inside each section so each expense gets its own native cell and swipe action.
- Accessibility sizes required wrapping rows and titles, semantic text styles, and symbol actions in sheet navigation bars. The verifier identified layout problems precisely.
- Reloading an edit route initially lost its form draft. Persisting the draft fields fixed that while each add/edit action still initializes the correct record.
- Real-touch swipe testing required a gesture starting near the row’s trailing edge and remaining inside the viewport. Targeting the list surface avoided the driver’s hit-target guard for the native cell accessory.

## What would have made this faster

A ready-made dated expense-history pattern, a documented way to emit multiple native list cells from a child component, and a high-level native full-swipe driver command would have reduced iteration. A precompiled toolkit would also shorten the cold start.

## Unfinished work

None of the requested features remain unfinished.
