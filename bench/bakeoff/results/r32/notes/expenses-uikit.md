# Expenses — implementation notes

Finished, built, installed, and launched as `dev.bakeoff.expenses.bcfcdc2` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. No other simulator was used.

## What shipped

Programmatic UIKit app/scene delegates, native navigation and tabs, day-grouped transaction history with daily totals, current-month summary, adding and editing all fields, swipe deletion, seven category totals and percentage shares, and monthly category drill-down. Search also matches merchants, categories, notes, and ISO dates. The supplied JSON is bundled unchanged in `Resources/transactions.json`.

Saved changes use exact decimal currency values and atomic JSON writes in Application Support. Seeding happens only when no saved file exists, including when the saved history is empty. Load failures preserve the existing file and block writes. Forms validate positive amounts and required merchants, provide keyboard dismissal, and protect unsaved changes. The interface adapts to dark appearance and accessibility text sizes.

## What was hard

- iOS 26 places large navigation titles within the scroll view; an opaque scroll-edge navigation background hid the title. A transparent scroll-edge appearance fixed it.
- Initial custom font construction applied accessibility scaling twice. Maximum-size screenshots exposed this; fonts now scale once from a standard-size base. Rows and date controls switch to vertical layouts where needed.
- macOS Bash with `set -u` rejects expansion of an empty flags array. The build script now always has a nonempty compiler-flags array.
- Simulator operations needed sandbox escalation to communicate with CoreSimulator. All operations remained limited to the assigned UDID.

## Decisions and lookups

Used the skeleton README and local compiler feedback; no external documentation or web research was needed. Category drill-down shows the current month, matching the category summary, and says so explicitly. Dates use local calendar days and dollar formatting stays US currency. Amounts must represent positive values with at most two nonzero fractional decimal places; malformed numbers and sub-cent values are rejected. Merchant is required; note is optional.

## Verification

- 119 simulator data/controller checks passed: seed integrity, exact totals, invalid and valid amounts, date grouping, category ordering, search/empty results, atomic storage, edits, empty-history relaunch, and corrupt-file preservation.
- 15 UIKit form-flow checks passed: defaults, category options, validation, add/edit fields, immediate list updates, date regrouping, swipe-action configuration, and deletion synchronization.
- Two separate process-relaunch checks passed for persistence and fixture cleanup.
- Reviewed simulator screenshots in light/dark appearance and at the largest accessibility text size, including scrolled transaction/category rows, date/note controls, and the decimal keyboard.
- Final stored history was compared with the supplied JSON: all 240 records match, with no test fixtures remaining.
- Final default build excludes integration-check code and compiles without warnings. Screenshots and check reports are in `build/`.

Optional integration checks are compiled with `EXPENSES_TESTING=1`; launch arguments are documented directly in `Sources/IntegrationChecks.swift`. The ordinary requested build command produces the finished app.

## Remaining work and what would help

No specification features remain unfinished. A supplied simulator interaction driver and a standard UIKit integration-check harness would have made UI verification faster than building launch-argument checks and collecting screenshots manually.
