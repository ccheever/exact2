# Expenses — build notes

Finished and installed on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`, with bundle ID `dev.bakeoff.expenses.bee503e`. The final build was launched using the supplied `build.sh --run` command. No requested feature remains unfinished.

The app is entirely programmatic UIKit. It bundles all 240 original transactions, uses integer cents for exact money calculations, and saves changes atomically to Application Support. The seed is loaded only when no saved history exists, so deleting everything does not resurrect the starting data. The original JSON is copied unchanged into Resources and the final app bundle.

The main list groups expenses by local calendar day, newest first, with daily totals and a current-month summary. Categories show monthly totals and percentages in descending spending order. Category details default to this month and include an All time option. Add/edit supports all seven categories, a native calendar, and notes. Search covers merchant, category, note, and displayed amount. Swipe deletion and editor deletion both persist.

## What was hard

- Making self-sizing table headers open at the correct position with native large navigation titles. Categories now explicitly starts at the top on its first appearance.
- Supporting the largest accessibility text size without clipping. Rows switch to vertical layouts, labels wrap, and summary currency fits the available width. An initial double-scaling error in weighted fonts was found and fixed during simulator review.
- Adapting tests to this simulator's native compact date picker, which exposes a button labeled “Date Picker,” rather than an XCTest DatePicker element.
- The skeleton had no Xcode project or UI test target. A small project was added for simulator UI tests; the supplied shell build remains the final app build path.

## Lookups and assumptions

I inspected the supplied README, seed data, `xcodebuild -help`, and simulator accessibility hierarchies. No web research or third-party libraries were needed.

Dates are stored as date-only Gregorian calendar keys in the local time zone. “This month” means the device's current calendar month. Merchant is required. Amounts must be positive USD values with at most two decimal places; the supported maximum is $9,999,999,999.99. Category colors, icons, the warm visual palette, and the receipt app icon were chosen during implementation.

## Validation

- Foundation checks passed for seed count/categories, exact monthly/category totals, invalid and zero amounts, cent precision, localized decimal separators, date round trips, month boundaries, add/edit/delete persistence, and empty-history persistence.
- Four end-to-end simulator UI tests passed: cancellation/search, native calendar selection, add/edit/category drill-down/swipe delete/relaunch persistence (including notes), and screen layouts.
- A final additional UI check passed asserting the Categories monthly total is visible on first opening.
- Reviewed simulator screenshots in light mode and dark mode, including the largest accessibility text size. Restored light mode and normal Large text size afterward.
- October's starting history reconciles to 86 expenses totaling $5,269.70. UI test expenses were deleted, and all 240 starting entries remain.
- Final build and installation succeeded. The temporary UI test runner was removed. The Expenses app remains installed and launched on the assigned simulator.

## What would make this faster

A UIKit skeleton with a ready UI test target would avoid creating the test project. A documented simulator interaction harness would also reduce native-control discovery. Disabling verbose failure diagnostics with `-collect-test-diagnostics never` significantly shortened UI test iteration after early selector failures.

Build again with:

```sh
SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
```

Tests live in `Tests/`; simulator results and screenshots are under `build/`.
