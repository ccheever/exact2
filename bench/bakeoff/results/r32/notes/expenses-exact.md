# Expenses build notes

Implemented the complete specification with Exact. The supplied 240 transactions are bundled as starting history (both the original JSON asset and an embedded seed module). Local writes use atomic JSON storage; money is stored as whole cents.

The app has a dated transaction list with day totals and a current-month summary, native add/edit sheets, seven-category selection, a date picker, optional notes, native swipe deletion, and a Categories tab ordered by current-month spending with shares and transaction drill-downs.

## What was hard

- The first shared compiler build needed permission to write Exact's toolchain cache. The iOS build then completed using the assigned simulator.
- A form inside a child component initially missed the native navigation-bar mapping. Moving the route's navigation attributes onto the child component's root fixed it, as the compiler warning recommended.
- Largest-text layout took several iterations. Toolbars needed wrapping in the web host, long one-word headings needed natural discretionary hyphens, and currency readouts use Exact's supported numeric fitting. The native form keeps its standard navigation bar and grouped controls.
- Editing while iOS verification ran caused a stale-build receipt. The final verification runs against a freshly installed, unchanged build.

## Lookups and decisions

I used Exact's guide, dashboard pattern, and vocabulary/recipes for edit sheets, tabs, swipe deletion, grouped forms, currency, dates, file storage, native navigation, and iOS driving. No external services are needed.

Category drill-downs show the selected category's transactions for the current month, matching the category totals. New expenses default to today and Dining. Merchant is required; notes are optional. Dates can be backdated or future-dated. Amounts must be positive whole-cent USD values below $10 billion. Date validation rejects nonexistent calendar dates.

## Validation

- Seven Contract behavior tests cover all seed records, ordering, daily/monthly totals, invalid amounts, add/edit/cancel, all input fields, category drill-down, deletion, month boundaries, and relaunch persistence.
- A separate data-module check reconciles category shares and totals, verifies one-cent amounts, invalid dates/categories, deleting all history without reseeding, and adding again afterward.
- Exact web verification: clean, including light/dark, phone/wide layouts, largest text size, and relaunch.
- Final iPhone verification: clean; 7/7 behavior tests passed, light/dark checks passed, largest accessibility text (53 pt body) had no spills, and relaunch persistence passed. Native list scrolling matched UIKit physics. The machine was heavily loaded, so the verifier did not judge frame timing.
- A final real-touch simulator drive confirmed the default history still has 240 transactions, October spending is $5,269.70, both tabs work, and the native decimal keyboard opens correctly. No test expense was saved to the default history.
- Built, installed, and launched as `dev.bakeoff.expenses.bbfa7d5` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

## What would make this faster

A complete expense-history starter, clearer examples of route attributes on child screen components, and keeping compiler/native caches warm would reduce setup and layout iterations. The verifier could distinguish a partially visible row at a scroll viewport boundary more clearly from content clipped by its own layout.

No specification feature was omitted. A local, generic Exact authoring diary was kept; it was not sent anywhere.
