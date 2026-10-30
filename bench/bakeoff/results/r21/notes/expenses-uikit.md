# Expenses build notes

Finished and installed as **Expenses** (`dev.bakeoff.expenses.a87bb3`) on simulator
`CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final app was built and launched with the supplied
`build.sh`. No other simulator was booted or used.

## Implementation and decisions

- UIKit throughout, with programmatic app/scene delegates, navigation controllers, tabs, tables,
  native category selection, and a native date picker. No SwiftUI or storyboards.
- All 240 original transactions ship unchanged in the app bundle and seed the history only once.
  Changes are written atomically to Application Support. Deleting every expense does not reseed
  the history. Failed saves leave the in-memory history unchanged.
- Amounts use exact integer cents and display as US dollars. Zero, negative, malformed, and
  sub-cent amounts are rejected. Merchant is required; notes are optional.
- I interpreted category drill-down as the transactions contributing to that category’s current
  month total. Future dates are allowed. Date-only storage avoids timezone shifts.
- Dynamic Type layouts put amounts below merchant/category text at accessibility sizes. The
  summary keeps its complete amount on one line. The form scrolls the note caret above the keyboard.

## What was hard / what I looked up

Self-sizing table headers, large text, and keyboard insets needed simulator inspection and
refinement. Standard navigation titles gave consistent results across tab changes. I checked
local `simctl ui` help for the exact appearance and accessibility text-size commands; no web
research or third-party dependencies were needed.

The small generated Xcode UI-test project needed a variable executable name in its plist;
otherwise a previously installed executable could be launched. The simulator also reports
calendar days inconsistently as buttons versus cells. Direct coordinate taps on the matched
calendar day resolved that test issue. A failed test run stalled while collecting diagnostics;
its collection process was stopped before the corrected test was rerun successfully.

## Validation

- Model checks passed: seed count, seven categories, exact monthly/category totals, currency,
  amount validation, invalid/leap dates, add/edit/delete, month boundaries, persistence, an empty
  history, corrupt saved data, and failed writes.
- Native UI checks passed: list and category navigation, positive-amount validation, required
  merchant, notes, add/edit, swipe deletion, relaunch persistence, discard confirmation, and
  editing dates across the October/November boundary.
- Final dark-mode / largest accessibility text checks passed, including category rows and typing
  a note with the keyboard open. Screenshots were inspected in `build/final-accessibility-screenshots/`.
- The final bundle’s transaction file matches the supplied data byte-for-byte. Test expenses were
  deleted, the test runner was removed, and the app was left launched in light mode at standard
  text size. `build/final-main.png` shows the final installed screen.

No specification features remain unfinished. A starter UI-test target and a simulator build/test
helper with executable-name handling already configured would have made this faster.
