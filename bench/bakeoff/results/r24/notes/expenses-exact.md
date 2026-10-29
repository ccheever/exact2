# Expenses build notes

Finished and installed as **Expenses** (`dev.bakeoff.expenses.bd5b468`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final app was launched with `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`.

## Implemented

- All 240 supplied transactions ship in the bundled data module as the initial history.
- Native grouped transaction lists, newest dates first, with date headings and day totals.
- Current-month spending summary and seven categories ranked by monthly spending, including percentage shares and category drilldowns.
- Native add/edit sheets with amount, merchant, category, date and optional note; positive-amount and merchant validation.
- Native swipe deletion and atomic local JSON persistence across relaunches.
- US currency formatting, integer-cent calculations, dark mode, Dynamic Type, and a custom receipt app icon.

## Decisions

- A category drilldown shows that category’s transactions for the current month, matching the month and total displayed above it.
- Categories with no spending remain visible with a zero share. An entirely empty month has seven zero shares.
- Date values are calendar dates, not UTC instants; selecting or reopening them does not shift the day with time zone conversion.
- Amounts round to the nearest cent. Values that round to zero are rejected. Future dates are allowed.
- Transactions sharing a date use descending numeric IDs as a stable order.

## What was hard or needed lookup

- Exact’s first native build compiled the pinned runtime and took several minutes. Later builds were quick.
- `formatDecimal` expects integer smallest units. Passing dollars instead of cents produced blank prefilled amounts; the vocabulary reference and edit tests identified the issue.
- A reusable modal form must put its navigation attributes on the component root that directly contains the header. A wrapper otherwise makes the header appear inside the sheet instead of as its native navigation bar.
- Native grouped section headers accept one text node, so day headings combine the date and total in one accessible string.
- Large-text checks found overflow in web tab labels, headings, and currency readouts. Wrapping tab layouts, native text styles, and Exact’s numeric readout fitting resolved them. Validation guidance is above the scrolling form so it stays visible.
- Lookups were limited to the pinned Exact guide, patterns, vocabulary, and driver help. No external services were needed.

## Verification

- `bun exact.mjs verify`: clean; five tests pass; light/dark, narrow/wide, persistence, and largest-text checks pass.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: clean; five tests pass; native light/dark, relaunch, and largest Dynamic Type checks pass.
- After the final currency readout adjustment, a targeted native category-detail check at body size 53 passed in both light and dark mode.
- The simulator scrolling check found nothing for the app to fix, with zero measured hitches and no dropped lift frames. The machine was heavily loaded, so the toolkit judged physics rather than absolute timing. It separately identified its own status-bar scroll-to-top timing difference.
- Tests cover seed counts and totals, validation, all saved fields, cancellation, monthly boundaries, edit prefill for different rows, category edits, swipe deletion, empty-month percentages, and relaunch persistence.
- Screenshots and detailed verification output are in `.exact/verify/`; the final targeted large-text screenshot is `.exact/final-category-large.png`.

## Unfinished work and what would make this faster

No requested app features remain unfinished. Only the specified simulator was used.

A warm native build cache, a complete expense-tracker pattern, clearer integer-unit formatting examples, and a reusable modal-form example with route attributes on its root would shorten the build. A quieter machine would make native performance measurements more conclusive.
