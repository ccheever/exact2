# Expenses build notes

Implemented the full supplied transaction history, daily sections and totals, current-month summary, seven-category breakdown with shares, category transaction lists, add/edit sheets, swipe deletion, and persistent local storage. The original `data/transactions.json` is imported into the shipped data module as the starting history. Money is stored as integer cents; writes are atomic, and deleting every item does not restore the seed history.

## Decisions and lookups

- Used Exact’s dashboard starter and the tabs, edit-sheet, grouped-form, dates-and-days, money, files-and-grants, and swipe documentation.
- Category drill-downs show the selected category’s current-month expenses, matching the scope of the Categories screen.
- Equal-date transactions use descending IDs for a stable order. Equal category totals use alphabetical order.
- Expenses added from a category’s screen default to that category. Other new expenses default to Groceries and the current local date.
- Required a merchant and a positive amount rounded to cents. Notes are optional. Past and future dates are allowed.

## What was hard

Exact requires section headers to contain one text node, so the date and daily total share a header label. Reusable form components need their navigation key and modal presentation on their own root to produce the native navigation bar. The first native build needed several minutes for shared Rust and Swift dependencies, and toolkit compilation required sandbox permission for the shared checkout.

The browser’s largest-text checks reported clipping of large single-word titles and the long tab label even with hyphenation enabled. Native iPhone verification is the release check for this iPhone app; its results are recorded below.

## What would make this faster

A ready-to-use seeded, grouped, editable transaction-list pattern; a reusable native sheet example with navigation attributes on the component root; and precompiled native dependencies would reduce setup and layout iteration.

## Validation

- Seven behavior tests cover all 240 starting entries, exact totals, amount validation, form defaults, optional notes, editing a second transaction, changing months/categories/dates, deletion, persistence, empty months, zero shares, and local midnight.
- Web behavior tests pass; normal-size light and dark layout checks pass.
- Native preview checks pass in light and dark mode.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: **clean**. Seven tests passed, light/dark checks passed, all nine checked screens fit at accessibility-extra-extra-extra-large (53-point body text), and relaunch persistence passed.
- Real-touch scrolling found nothing for the app to fix. Exact reported a toolkit-owned status-bar scroll-to-top timing difference; the machine was busy, so timing was not judged.
- Real-touch smoke check confirmed the decimal keyboard, keyboard dismissal control, merchant field, and native calendar date picker.
- Final `bun exact.mjs ios --run` built, installed, and launched **Expenses** (`dev.bakeoff.expenses.bf77682`) on simulator **EEFC0029-1010-45E4-8242-42CF67E16C96**. No other simulator was used.
- No unfinished iPhone features or known app-level native layout problems.
