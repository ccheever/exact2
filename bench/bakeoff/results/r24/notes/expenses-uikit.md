# Expenses — build notes

Finished and installed as **Expenses** (`dev.bakeoff.expenses.bfafa26`) on simulator **CEDFB9C9-FF04-4617-B156-94C56B83CA86**. No other simulator was booted or used.

## What shipped

- Programmatic UIKit app delegate, scene delegate, tab navigation, grouped transaction history, monthly summary, and monthly category breakdown with drill-down.
- Add/edit sheets with a decimal keyboard, seven-category picker, native date picker, optional multiline notes, validation, and protection against accidentally dismissing unsaved changes.
- Swipe deletion, immediate summary updates, US currency, Dynamic Type through the largest accessibility size, VoiceOver labels, and light/dark appearances.
- All 240 original transactions copied unchanged into `Resources/transactions.json` and included in the app bundle. The history is seeded once; subsequent changes are stored atomically in Application Support. An intentionally empty saved history stays empty. An unreadable saved file is preserved rather than overwritten.
- A custom receipt app icon, generated locally by `Tests/make_icon.py`.

## What was hard

The largest text size exposed double scaling in weighted custom fonts: their base sizes were already scaled before passing through UIFontMetrics. Using default-size base fonts fixed that. The list rows change to vertical amount layouts at accessibility sizes, date controls stack vertically, and the summaries omit decorative content to keep navigation practical. Simulator screenshots were used to verify the fixes.

Exact money arithmetic and persistence were kept separate from presentation. Amounts use Decimal, and the in-memory history changes only after a successful atomic file write. Date strings remain calendar dates, so seeding does not shift them through UTC conversion.

Simulator presentation animations needed a longer settling interval for the keyboard-focus check. The form itself opened the decimal keyboard correctly; the initial check ran too early.

## Lookups and decisions

Read the supplied README and build script, inspected the JSON schema and category names, and used `simctl ui` help for appearance and accessibility-size settings. The compiler identified the iOS 26 replacement of the old bar-button `.done` style with `.prominent`. No web research or external dependencies were needed for the app.

With no questions available, I chose:

- “This month” means the device's current calendar month. The supplied October 2026 history totals $5,269.70 across 86 expenses.
- A category drill-down shows that category's transactions for the month shown in its header.
- Same-day transactions use descending IDs as a stable tie-breaker because the data has dates but no times.
- Merchant names are required and trimmed. Amounts must be positive with at most two decimal places; both decimal-point and decimal-comma entry are accepted, including `.50` and `12.`. Notes are optional. Dates may be past or future.
- New entries initially select Dining, and all seven categories remain available even when a category's monthly total is zero.

## Verification

`Tests/run-checks.sh` runs **34 model checks** and **27 simulator UIKit checks**, then reinstalls and launches the production build. All passed. Model checks cover starting data, exact totals, input validation, add/edit/delete persistence, month boundaries, empty saved histories, and preserving corrupt files.

The simulator checks invoke the same UIKit handlers used by the controls, including form validation, first-responder keyboard behavior, category selection, save/dismiss, edit prefill, date/category changes, swipe-delete callback, immediate list updates, persisted reloads, category sorting, drill-down filtering, and empty states. The temporary test transaction is deleted and the original 240 records restored. The test harness is excluded from the production build.

Visually inspected history, category breakdown, transaction rows, category rows, and form controls in light/dark mode and at the largest accessibility text size. Screenshots and test logs are in `build/`. The final bundle's identifier, name, scene manifest, and exact copy of the 240-transaction seed were also checked. The final app was rebuilt and installed after the checks.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A preconfigured simulator gesture/accessibility driver would have shortened interaction testing. The supplied direct-swiftc build script was useful; a ready-made UIKit scene setup and a native UI test target would have reduced initial setup and test-harness work.
