# Expenses — build notes

Finished, built, installed, and launched as `dev.bakeoff.expenses.b417e78` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

## What ships

- All 240 supplied transactions from `data/transactions.json` are bundled as the starting history.
- Transactions appear newest first, grouped by date with day totals. The summary shows current-month spending.
- Add and edit sheets support amount, merchant, all seven source categories, date, and an optional note. Amount validation rejects malformed, zero, and negative values.
- Native swipe deletion, category totals ranked by spending, percentage shares, and current-month category transaction lists work.
- Amounts are stored in integer cents and displayed in US currency. An atomic local ledger preserves additions, edits, and deletions across relaunches. An empty saved ledger stays empty.

## Verification

`bun exact.mjs verify ios` completed cleanly: eight tests passed; first screen, test ends, and relaunch passed light/dark checks; all ten screens checked at the largest accessibility text size had no spills. The native touch scrolling check reported “like UIKit,” with zero measured hitches. Machine load prevented a final timing judgment; scrolling physics passed.

The tests cover seed ordering and totals, invalid amounts, all saved fields, independent editor drafts, moving an expense into another month, category drill-down and editing, deletion, empty-month behavior, and relaunch persistence.

The web feature suite also passed all eight tests. Its extreme-text checks still report partially visible content at modal scrolling boundaries; the requested iPhone target passes its native checks.

## What was hard and what I looked up

The main difficulty was native sheet navigation. A header inside an extra component wrapper initially looked acceptable at normal size but overlapped fields at the largest Dynamic Type size. Exact’s `header` reference explained that the editor component’s root must carry the route’s `navigationKey`. Moving the route boundary there produced a proper native navigation bar and fixed the accessibility failures.

I used Exact’s dashboard starter and looked up edit sheets, tabs, grouped forms, date inputs, swipe deletion, file grants, money formatting, adaptive layouts, and simulator driving. `formatDecimal` takes integer smallest units, so the editor passes cents directly.

I treated category drill-downs as current-month lists, matching the category summary. Same-day transactions use descending IDs as a deterministic tie-breaker. Dollar inputs round to cents; values beyond safe integer cents are refused.

## Unfinished work

No requested iPhone features remain unfinished.

## What would have made this faster

- An edit-sheet starter that puts the navigation key on the child component’s root.
- A nested-header diagnostic that follows component boundaries.
- Clearer distinction in the web accessibility report between clipped content and content partly visible at a scroll boundary.
- A warmer initial compiler/native build cache and a pattern command that recognizes the untouched generated starter.
