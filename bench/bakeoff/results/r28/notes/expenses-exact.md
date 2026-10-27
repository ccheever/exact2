# Expenses build notes

Implemented and installed Expenses (`dev.bakeoff.expenses.b95f66b`) on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final build was launched with `bun exact.mjs ios --run` using that simulator explicitly. No other simulator was used.

## What ships

- All 240 supplied transactions are bundled through the imported JSON as the initial history.
- Transactions are sorted by descending date, grouped by day with day totals, and have native swipe deletion and edit sheets.
- The monthly summary and category totals use integer cents and US currency formatting.
- All seven categories are available in the expense form. Category drilldowns show the current month's transactions, consistent with their monthly totals and percentages. Adding from a category preselects it.
- Adds, edits, notes, dates, and deletions persist in an atomic local JSON file. An existing empty file remains empty rather than restoring the seed history.
- A custom app icon is included.

## Verification

`EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios` finished **clean**:

- Six tests passed, covering the seed history, daily/monthly totals, category order and shares, invalid amounts, adding, editing each field, independent edit drafts, cancellation, deletion, empty months, and relaunch persistence.
- No findings on the initial screen, six test endpoints, or relaunch in light or dark mode.
- All eight checked screens fit at the largest accessibility text size (53 pt body).
- Real-touch scrolling reported “like UIKit.” The machine was heavily loaded, so Exact judged scrolling physics rather than timing.

The web verification also passed all six behavior tests and normal-size layout checks. Its extreme 53 px text preview still reports three web-only findings involving long navigation titles and a partially visible form label. The iPhone target has no remaining verification findings.

## What was hard / looked up

- Used Exact's dashboard starter, tabs pattern, and recipes for grouped forms, edit sheets, local files, money, dates, assets, and simulator driving.
- The initial compiler needed permission to populate the toolkit's build cache, and the first native build compiled the host dependencies.
- `formatDecimal` expects integer cents plus a decimal scale. I initially divided by 100; tests caught blank edit amounts, and the money recipe resolved it.
- An extra component wrapper prevented the form header from becoming a native navigation bar. It looked plausible at normal size but overlapped fields at accessibility sizes. Moving the navigation attributes onto the form component's root fixed this and produced proper native sheet bars.
- The compiler caught a literal route shadowed by a parameter route, plural labels, and section-header restrictions.

## Decisions and limitations

- Same-day transactions have no timestamps in the supplied data, so their IDs provide a deterministic descending tie-break.
- Dollar inputs are rounded once to whole cents. Zero, negative, malformed, and non-finite amounts cannot be saved.
- Category percentages use one decimal place; an empty month shows zero shares.
- Saved transaction data persists; unfinished drafts are not preserved across relaunch.
- No requested iPhone feature remains unfinished.

## What would make this faster

- A diagnostic when a navigation screen's header is hidden behind an extra component wrapper.
- An explicit edit-sheet example with navigation attributes on the form component's own root.
- More direct web-preview guidance for extreme-size headers and list clipping.
- Warm native and compiler caches for the first build.
