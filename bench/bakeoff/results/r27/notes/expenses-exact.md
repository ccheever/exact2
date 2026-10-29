# Expenses build notes

Finished and installed as `dev.bakeoff.expenses.bae2d61` (Expenses) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final standalone build was launched successfully with `bun exact.mjs ios --sim … --run`.

## Implemented

- All 240 supplied transactions are bundled from `data/transactions.json` as starting history.
- Newest-first history, grouped by calendar day with daily totals, plus the current month’s total.
- Add and prefilled edit sheets with merchant, positive amount, seven-category picker, native date picker, and optional note.
- Native swipe deletion, ranked monthly categories with spending shares, and category transaction drill-down.
- Atomic local persistence of additions, edits, and deletions. An intentionally empty saved history does not restore the seed.
- System navigation, sheets, decimal keyboard, semantic colors, dark mode, and Dynamic Type layouts.

## Decisions

Category drill-down shows the selected category’s transactions for the month represented by its summary. Transactions sharing a date are ordered by descending ID. Amounts are stored as whole cents and formatted as USD; entry rounds to cents and has a stated maximum below $1 billion. New expenses use the device’s current local calendar day, read when opening the form.

## What was hard / looked up

Exact’s dashboard starter supplied the resource and storage foundation. I consulted its edit-sheet, money, date, grouped-form, navigation, asset, swipe, and simulator-driving references.

The main pitfalls were native navigation semantics: a toolbar action needs a button rather than a link, and a reusable sheet component must put navigation attributes on its own root. The compiler and native tree made both issues clear. Cold Rust/Swift builds took several minutes; shared compiler/cache access required sandbox escalation. Incremental builds were fast.

## Validation and remaining limitations

Final `verify ios` was clean: seven feature tests passed; light/dark layout checks found no issues; nine screens at accessibility-extra-extra-extra-large had no spills; relaunch retained saved history. Real-touch category navigation and editing also passed. Native scrolling found nothing for this app to fix. Exact noted a toolkit-owned status-bar scroll-to-top difference and heavy machine load, so absolute timing was not judged.

Web verification also passed all seven tests and ordinary phone/wide layout checks. At the largest web font size it reports two partially visible form labels/footers at scroll boundaries. The shipping iPhone app’s corresponding native accessibility checks are clean. No requested iPhone feature remains unfinished.

## What would have made this faster

A combined grouped-history/add/edit/category starter, a clearer toolbar-link diagnostic, and early build-progress heartbeats would reduce iteration time. The semantic content parts and automated native accessibility checks were especially useful.
