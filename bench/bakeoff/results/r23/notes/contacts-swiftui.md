# Contacts build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.b366171`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final installation starts with the original 320 people, including 31 favorites. `Resources/contacts.json` is an exact copy of the supplied data and is included in the app bundle.

## Implementation and choices

- SwiftUI navigation, searchable lists, native alphabetical section index, grouped details, and modal contact forms.
- Favorites use a horizontal section at the top and remain in the complete alphabetical directory. Counts refer to unique people.
- Contacts without a last name sort and group by first name. Accented initials group with their base letter; nonletter initials use `#`.
- Search handles names, companies, email, case/diacritics, and phone numbers with or without formatting.
- Names are trimmed, and whitespace alone cannot satisfy the first-or-last-name requirement. Editing preserves birthdays and favorite status.
- Every change is written atomically to the app’s Documents directory before the visible store updates. A saved empty directory stays empty on relaunch.
- Deletion and discarding edits require confirmation. Phone/email use system URL actions and offer copying through a long press; unavailable handlers show an explanation.

## What was hard or needed checking

Keyboard avoidance at the largest accessibility size needed explicit scrolling to the focused field and room for the keyboard toolbar. Screenshot review also caught inherited tint reducing secondary-label contrast in dark mode. Email display now permits wrapping without adding visible hyphens.

I inspected the installed SwiftUI SDK interface to confirm the native `sectionIndexLabel` API. I created a separate lightweight Xcode UI-test runner because the skeleton builds the app directly with `swiftc`. Disabling automatic failure-diagnostic collection avoided long simulator test-report delays.

## Validation

- Model checks passed: all 320 seeds, ordering, search, name validation, first/last-only names, editing, favorites, deletion, persistence of an empty store, and failed-write rollback.
- Simulator UI tests passed: add/edit, whitespace validation, favorite/unfavorite across relaunch, name/email/company/normalized-phone search, clearing search, deletion from both list and details, and persistence across relaunch.
- Native index test passed: tapping Z reached the Zhang contacts and opened James Zhang.
- Dark mode at the largest accessibility size passed browsing, details, editing with the keyboard, and discarding changes. Screenshots were inspected, including the focused field above the keyboard.
- Final normal-size workflow regression passed after the keyboard refinements. The final build compiled, installed, and launched successfully.

No specification features remain unfinished. Actual calls and outgoing email depend on system handlers, which the simulator cannot fully exercise.

A preconfigured simulator UI-test target and an example of the supported native section-index API would have made this faster. Test sources remain in `Tests/`; logs, screenshots, and result bundles are in `build/`.
