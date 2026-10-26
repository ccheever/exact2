# Expenses — implementation notes

Finished and installed as `dev.bakeoff.expenses.b132ed2` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final build was launched without debug test hooks. No other simulator was used.

## Delivered

- Programmatic UIKit app delegate, scene delegate, navigation controllers, and two native tabs.
- All 240 original transactions shipped in `Resources/transactions.json` and seeded once.
- Newest-first day groups with daily totals, this month's spending summary, editable transactions, and swipe deletion.
- Add/edit form with amount, merchant, seven-category picker, native date control, and optional note. Validation rejects zero, negative, malformed, and sub-cent amounts. Keyboard toolbar, focus scrolling, and unsaved-draft protection are included.
- Sorted monthly category totals, percentages, a category chart, and category transaction drilldown. Adding from a category defaults to that category.
- Exact Decimal arithmetic and atomic JSON persistence in Application Support. Deleting all history does not restore the seed on relaunch.
- Adaptive light/dark colors, Dynamic Type, accessibility labels, expanded transaction rows at accessibility sizes, and a custom app icon.

## Verification

27 in-app simulator assertions passed. These cover the original count/categories/October total, numeric validation, leading decimals and decimal commas, daylight-saving date round trips, descending day grouping, swipe-delete configuration, actual form save/edit paths, trimmed merchants and notes, month-boundary edits, reload persistence, delete persistence, category ordering and filtering, empty histories, deleting the final expense, exact decimal addition, and header measurement.

Simulator screenshots were inspected for Activity, Categories, category drilldown, the expense form with its numeric keyboard, and dark mode at the largest accessibility text size. The date and note controls were checked after scrolling the largest-text form. Evidence is under `build/`; the assertion report is `build/self-tests.txt`.

After the final non-debug build was installed, the app was terminated and relaunched. Its persisted file still contained exactly 240 expenses and an October total of $5,269.70.

To run the internal checks again against the original starting history:

```sh
APP_DEBUG=1 SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run
xcrun simctl terminate CEDFB9C9-FF04-4617-B156-94C56B83CA86 dev.bakeoff.expenses.b132ed2
xcrun simctl launch CEDFB9C9-FF04-4617-B156-94C56B83CA86 dev.bakeoff.expenses.b132ed2 --self-test
```

The default build excludes all test-entry behavior.

## What was hard

UIKit's large navigation title appeared blank in an early layout. Explicit navigation-item titles and an inline title for Categories resolved it. Large accessibility fonts also split currency totals across lines; summary amounts now fit on one line while transaction rows switch to a vertical layout. The scrollable form keeps every control reachable with large text and the keyboard.

The host's Bash treats an empty array as unbound under `set -u`; the optional debug build flag now uses an explicit compilation condition instead.

## Decisions and lookups

Read the supplied README, build script, specification, and transaction data. No external dependencies or web research were needed. Assumed spending means all transactions dated within the current local calendar month, category drilldowns show that same month, merchants are required, and US currency entry uses cents. Category shares are rounded to whole percentages. The default category for a general new expense is Dining. Date-only strings preserve the selected calendar day.

## Remaining work and ways to go faster

No specified feature remains unfinished. The toolkit offers simctl and custom checks, so form operations were exercised through their actual controller methods and layouts through screenshots; there was no external touch-gesture automation or XCTest runner. A ready-made UI gesture driver would make swipe, keyboard, and tab interaction testing faster. A starter UIKit scene setup and app-icon pipeline would also reduce setup time.
