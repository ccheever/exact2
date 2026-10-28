# Expenses build notes

Completed and installed as `dev.bakeoff.expenses.b995e3f` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The app is left open on its main transaction screen. No other simulator was used.

## Implementation

- Programmatic UIKit app and scene delegates, native navigation, tab bar, grouped tables, modal expense editor, category menus and date picker.
- All 240 original transactions ship unchanged in the bundle. The first launch copies history into Application Support; later launches load saved history, including an intentionally empty history.
- Add, edit, swipe delete, day totals, monthly totals, category percentages, and monthly category drill-downs are implemented. Saves are atomic and update visible screens immediately.
- Decimal input and integer-cent aggregation avoid floating-point currency errors. Entries must be positive, have at most two decimal places, and fit within twelve whole-number digits. Merchant is required; note is optional.
- Dark mode, Dynamic Type through the largest accessibility size, VoiceOver labels, keyboard avoidance, and unsaved-change confirmation are included.

## Hard parts and decisions

- Keeping date-only records stable across local time and daylight-saving boundaries, and making monthly summaries respond correctly when an edited expense moves into another month.
- Avoiding broken horizontal layouts at the largest text size. Amounts and descriptive text stack vertically there; headings and summary amounts have appropriate scaling limits.
- The simulator's native expanded navigation title did not render reliably with this table-header configuration. The roots use accessible scrolling large headings and standard navigation titles when scrolled instead.
- Interpreted the category drill-down as this month's transactions, matching the category totals and percentages. All seven categories remain visible even if their spending is zero.
- No web research was necessary. The skeleton README, compiler diagnostics, public UIKit properties and simulator screenshots supplied the needed feedback.

## Verification

The final installed build passed 37 simulator-based checks; see `artifacts/test-results.txt`. These cover seed integrity, exact October total ($5,269.70), chronology, date round trips, invalid and fractional amounts, editor validation, live list refresh, add/edit/delete persistence, moving expenses between months, empty history, category reconciliation and navigation, and the swipe-delete action configuration.

Screenshots in `artifacts/` document light and dark layouts, the largest text size, the keyboard-open note editor, category detail, and the finished main screen. The bundled transaction JSON was compared byte-for-byte with `data/transactions.json`, and the final Info.plist passed validation. Tests use a temporary store and leave the user's starting history untouched.

Rebuild and launch:

```sh
SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
```

Run the in-app checks on the installed build by terminating it and launching with `--self-test` using `xcrun simctl`. The report is written to the app's Documents/test-results.txt. Additional screenshot launch flags are `--show-add`, `--focus-amount`, `--focus-note`, `--show-edit`, `--show-categories`, `--show-detail`, and `--review-list`.

## Unfinished work and tooling

No specification features remain unfinished. A ready-made simulator touch/gesture driver and a UIKit scene-manifest starter would have made the work faster. UI verification here used simulator launch routes, programmatic controller checks, and screenshots rather than a separate UI-test runner.
