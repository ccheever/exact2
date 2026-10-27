# Expenses build notes

Finished and installed as `dev.bakeoff.expenses.b94dbc4` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final install used `bun exact.mjs ios` with that simulator explicitly selected.

## Delivered

- All 240 original transactions are bundled from `data/transactions.json` as the starting history.
- Newest-first history, daily totals, current-month spending, native add/edit sheets, swipe deletion, seven categories, category totals/shares and monthly transaction drill-down.
- Atomic local JSON persistence, whole-cent arithmetic, US currency formatting, native date/category controls, dark mode, accessible text sizes, and a receipt app icon.

## Verification

- Final `verify ios`: **clean**, 7 tests passed, 0 failed.
- Light and dark screen checks: no findings.
- Largest accessibility text size: nothing spills across nine checked screens.
- Native scrolling: “like UIKit.” The verifier noted heavy machine load and judged physics rather than timing.
- Three additional data tests passed with 29 assertions, including reconciliation of seed totals, invalid writes/dates, and deleting the entire history without reseeding it.
- October seed totals independently checked: $5,269.70 over 86 transactions; October 25 totals $222.02.

## What was hard / looked up

The first cold native build took roughly ten minutes; later builds took seconds. I looked up Exact’s modal forms, tabs, grouped-list restrictions, swipe deletion, dates, money, persistence grants and native driving commands.

The main presentation issue was a header nested inside a form component: it looked plausible at ordinary size but rendered as authored content and grew excessively at accessibility sizes. Moving it directly onto the modal route produced UIKit navigation-bar controls. Built-in RowText also provided better trailing-value alignment and accessibility stacking than a custom row.

Native relaunch returns to the list, while web reload can retain the editor route. The tests now reopen the saved expense appropriately, and saved editor routes restore their values on the web.

## Decisions and limitations

Category drill-down shows the current month, matching the category summary; the Expenses tab contains the complete history. Same-day entries sort by descending ID because the data has no time of day. Amounts round to cents, must be positive, and individual entries are capped at $9,999,999.99.

No requested iPhone feature remains unfinished. The separate web audit still reports three scroll-edge clipping findings in the editor at its largest simulated text size; the shipped iPhone audit is clean.

## What would make this faster

A warm native build cache, explicit documentation about header promotion through child components, and examples covering grouped-section header/footer restrictions would shorten the build substantially. Keeping native builds, source edits and verification sequential also avoids stale-build refusals.
