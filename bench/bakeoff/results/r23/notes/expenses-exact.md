# Expenses

Finished, built, installed, and launched on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

- App name: Expenses
- Bundle ID: `dev.bakeoff.expenses.bdd69be`
- Final install: `bun exact.mjs ios --run --sim EEFC0029-1010-45E4-8242-42CF67E16C96`

## Implemented

The original 240 transactions are bundled through the data module as the starting history. History is newest first, grouped by date with daily totals. The monthly summary, seven ranked categories with percentage shares, category history, validated add/edit sheets, swipe deletion, and persistent local storage are implemented. Currency calculations use integer cents; deleting everything does not restore the seed data.

## Validation

`bun exact.mjs verify ios --sim EEFC0029-1010-45E4-8242-42CF67E16C96` finished clean: 8 tests passed, none failed. Native light/dark layout checks and the largest accessibility text size found no clipping. Tests cover initial totals, invalid/zero/negative amounts, all form fields, edits across month boundaries, fresh drafts for different records, deletion, relaunch persistence, category ranking, and local midnight.

The native scrolling check found nothing for the app to fix. It reported that machine load was too high to judge performance timing, and identified a toolkit-owned status-bar scroll-to-top difference.

`bun exact.mjs verify` also passed all 8 functional tests and standard light/dark layouts. At its extreme 53 px web text size it reports two partially visible edit-form labels at the scroll viewport boundaries. The native iPhone accessibility check is clean.

## What was hard

- The first build needed the toolkit's compiler/native caches outside the app workspace and several minutes of Rust/Swift compilation.
- Refreshing a sectioned list automatically while its edit sheet was open left a surviving native row untappable after moving the top expense to another date. Waiting and changing list/section identities did not solve it. Refreshing history from the successful-save callback, together with closing the sheet, fixed the issue. The regression test now immediately opens another row after that edit.
- Sheet navigation metadata must be on the form component's root so its header becomes the native navigation bar. The compiler warning helped catch the initial nested structure.

## Lookups and choices

I used Exact's local guide and recipes for tabs, grouped forms, date pickers, edit sheets, swipe actions, currency, file grants, mutation completion, and simulator driving. No external research was needed.

Category details show all historical transactions in that category, while their summary remains this month's spending. Same-day transactions use descending numeric IDs as a deterministic order. Dates from the supplied history are preserved; new dates default to the local day when the sheet opens. Amounts are rounded to cents and must remain positive and safely representable as integer cents.

No requested iPhone functionality is unfinished.

## What would have made this faster

A tested example that moves a transaction between date sections while dismissing an edit sheet, and guidance on refreshing native list resources after dismissal, would have avoided the main debugging loop. The tested starters and native layout/accessibility checks were useful.
