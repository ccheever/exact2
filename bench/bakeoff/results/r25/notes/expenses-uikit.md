# Build notes

Finished Expenses as a programmatic UIKit app, bundle ID `dev.bakeoff.expenses.b5a93d0`. The final non-debug build is installed and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## Implemented

- All 240 original transactions are bundled, seeded once, and listed newest first with daily dates and totals.
- Current-month total, seven category totals sorted largest first, percentage shares, a ring chart, and category drill-down.
- Add, edit, and swipe delete. Amount, merchant, category, date, and optional note are saved locally with atomic writes.
- Positive amounts with at most two decimal places; amounts are stored as integer cents and displayed as US currency.
- Native navigation, category menus, date picker, keyboard dismissal, inline errors, unsaved-change protection, Dynamic Type, VoiceOver labels, and dark mode.

## What was hard

The supplied direct-swiftc environment rendered UIKit's large navigation titles as empty space on this simulator. Compact native navigation titles rendered correctly, so the finished app uses those. Self-sizing table headers and the largest accessibility size needed explicit width fitting and vertical row layouts. Long notes needed caret-based scrolling to remain visible above the keyboard.

Simulator installation required running the supplied build command outside the filesystem sandbox; the initial sandboxed invocation could not connect to CoreSimulator. Subsequent installs and checks succeeded.

## Lookups and choices

Read the starter README and build script, inspected the JSON schema and all category names, and checked simulator navigation hierarchy while diagnosing the title issue. No web research or third-party dependencies were needed.

The spec did not define a default category or amount limit: new expenses default to Dining (or the opened category), and amounts are limited to $999,999,999.99. Merchant is required. Dates use local calendar days without UTC conversion. Category drill-down shows the current month, matching its parent summary. Existing history remains visible across all months on Activity.

## Validation

- 42 model checks passed, including seed decoding, exact cent arithmetic, invalid amounts, month/date boundaries, create/edit/delete persistence, empty-history persistence, and corrupt-file handling. The tests use an isolated file and a fixed clock.
- 18 simulator integration checks passed through the real editor and store paths. Verified add/edit, zero/negative/fractional-cent and blank-merchant validation, saved fields, reload persistence, deletion, swipe action configuration, category ordering and totals, and drill-down filters.
- Reviewed simulator screenshots in light and dark appearances, at standard and largest accessibility text sizes, including transaction rows, category summary, editor, amount keyboard, and a 30-line note with the caret above the keyboard.
- Confirmed a normal final release-build terminate/relaunch. The original 240 records remain after test cleanup. October's independently calculated total is $5,269.70 across 86 expenses.

The simulator checks invoke the application's UIKit actions directly; physical swipe/tap gesture automation was unavailable in the provided toolkit. No required app feature is unfinished.

## What would have made this faster

A supplied simulator interaction tool or UI-test runner would have made menus and swipe gestures easier to verify. A known-good UIKit scene template and a note about the simulator's large-title rendering behavior would also have saved setup time.
