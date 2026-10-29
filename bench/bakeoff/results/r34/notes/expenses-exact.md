# Expenses build notes

Finished and installed as `dev.bakeoff.expenses.b94e472` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final `bun exact.mjs ios --run` succeeded and launched the app. No other simulator was used.

## Implemented

- All 240 supplied transactions are embedded as starting history through `data/seed.ts`, generated directly from the unchanged `data/transactions.json`.
- Newest-first history grouped by calendar day, with day totals and the current month's spending.
- Native add/edit sheets with positive amount validation, merchant, all seven categories, local-today date default, calendar date picker, and optional multiline note.
- Native swipe deletion, ranked monthly categories, percentage shares, and category transaction lists.
- Integer cents throughout, formatted as US currency. Atomic local JSON storage preserves additions, edits, and deletions, including an entirely emptied history.

## Decisions

Category drilldowns show the current month's transactions, matching the monthly totals and shares on their parent screen. History shows every month. Supplied date-only values stay calendar dates, without timezone conversion. A newly entered date defaults to the user's local day. Amounts are rounded to cents.

## What was hard and what I looked up

I used Exact's start guide, dashboard starter, and focused recipes for tabs, edit sheets, swipe actions, grouped forms, money, dates, persistence, mutation completion, and simulator driving. Native sheet headers must be direct children of the navigation root; the compiler identified and helped fix an initially nested header. Grouped section headers permit one text node, so each combines the day and its total.

The initial cold toolkit build needed access to the pinned checkout's shared build cache. Native compilation took longer than subsequent edits. Largest-text verification found several web layout constraints; flexible wrapping and semantic text styles resolved them. iOS layout checks passed at the largest accessibility text size.

## Verification

Seven functional tests passed on both web and iOS. They cover source-data totals and ordering, invalid amounts, required merchant, cancellation, all saved fields, relaunch persistence, independent edit drafts, moving an expense between months, swipe deletion, category ordering/shares/drilldown, and empty months.

Final iOS verification reported `verify: clean`: no light/dark findings, nothing spilling on nine screens at the largest accessibility text size, and nothing for the app to fix during real-touch scrolling. The machine was heavily loaded, so the toolkit judged scrolling physics rather than timing. It noted its own status-bar scroll-to-top difference from UIKit, outside app control.

The additional experimental synchronous oracle/differential checker emits synthetic-data assertion notes and engine divergence diagnostics after the native verification report. These are separate from the seven passing host tests and native appearance checks; the native app itself has no known unfinished specification features.

## What would have made this faster

A warm native build cache, an edit-sheet starter with navigation properties already placed on the child component root, and an automatic embedded-plan rebuild before the scrolling measurement would have avoided extra build/verification cycles. A concise mapping of standard SF Symbols to Exact's cross-platform roles would also help.
