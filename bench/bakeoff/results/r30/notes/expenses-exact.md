# Expenses — build notes

Completed and installed `dev.bakeoff.expenses.bc4998c` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` using `EXACT_SIM=… bun exact.mjs ios`.

## What shipped

- All 240 supplied transactions are compiled into the starting history. Subsequent changes are saved atomically in the app's private persistent storage.
- Transactions ordered newest first, grouped by date with day totals; current-month spending summary.
- Native add and edit sheets, seven-category picker, date picker, optional note, and positive-amount validation.
- Native swipe deletion; category totals ordered by spending, rounded percentage shares, and current-month category transaction lists.
- US currency formatting, integer-cent calculations, system dark mode, Dynamic Type, and a receipt app icon.

## What was hard and what I looked up

I used Exact's local start-here guide, dashboard pattern, and focused recipes for tabs, editing sheets, grouped forms, storage grants, currency, dates, swipe actions, and simulator driving. No external research was needed.

The main difficulty was the sheet's navigation container. A form component nested inside a route initially drew its header as page content. That looked reasonable at the normal text size but overlapped at the largest accessibility size. Moving the navigation key and sheet presentation onto the form component's own root made UIKit own its title and buttons and resolved the problem. The header reference explained this more clearly than the editing-sheet example.

The compiler also required section footers to be unconditional final children, and flagged redundant summary cards inside grouped cells. Both were corrected. Initial compiler setup needed the toolkit's shared build directory to be writable through the approved build command.

## Decisions

- Category drill-downs show the current calendar month's transactions, matching the category summary's scope; History retains all dates.
- Dates are calendar dates in the user's local zone, with today's date captured when opening Add.
- Amounts are rounded to cents and must remain positive after rounding. Individual amounts are limited to less than $1 billion.
- Category percentages are rounded to whole percentages, so their displayed sum can differ slightly from 100%.

## Verification

The final `bun exact.mjs verify ios` was clean:

- Seven functional tests passed, including seed totals, invalid amounts, adding, notes, persistence after relaunch, editing independent rows, date changes across months, swipe deletion, category filtering, canceled edits, and an empty month.
- Light and dark layout checks reported no findings.
- Nine screens at accessibility extra-extra-extra-large (53-point body text) had no spills.
- Real-touch scrolling found nothing for the app to fix. Exact separately reported its own slightly slower status-bar scroll-to-top behavior; this is a host issue rather than app code.

The final native build was installed after verification so normal Home Screen launches use the finished embedded plan. Only the assigned simulator was used.

## Unfinished work

No requested iPhone feature remains unfinished.

## What would make this faster

A corrected edit-sheet example showing the navigation key on the component root, an immediate warning for a nested non-native header, and clearer first-build sandbox guidance would avoid the main detours. Targeted accessibility-screen checks would shorten iteration between full simulator verification runs.
