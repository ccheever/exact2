# Contacts — build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.62079a`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final normal build is running. No other simulator was used.

## What shipped

- All 320 original contacts, copied unchanged into the app bundle as `Resources/contacts.json`.
- Last-name sorting, alphabetical sections, a tappable/draggable section index, and a horizontal favorites section.
- Live name/company/email/phone search, including case-insensitive and accent-insensitive matching and unformatted phone numbers.
- Contact details, known birthdays, phone/email actions, favorite toggling, adding, editing, and deletion from the list or details.
- Native forms, appropriate keyboards, keyboard Done controls, validation of trimmed first/last names, discard confirmation, and delete confirmation.
- Atomic JSON persistence in the app's Documents directory. An empty saved directory remains empty rather than restoring the seed. Failed writes leave the displayed data unchanged and show an error.
- Light/dark colors, Dynamic Type through the largest accessibility setting, VoiceOver labels, and a custom app icon.

## What was hard

The custom alphabet index needed to work with native List sections while preserving swipe actions. The largest accessibility text size exposed crowding in the directory header and an icon overlapping a details label. Those were corrected; decorative form content is omitted at accessibility sizes so the fields get the space.

The system Bash treats empty arrays as unbound with `set -u`. The optional test-build plumbing initially hit that case; it now uses nonempty compiler/source arrays, and the final normal build succeeds without warnings.

## Lookups and decisions

No web research or third-party dependency was needed. I inspected the supplied build script and the JSON's null fields, checked the designated simulator with simctl, and used compiler diagnostics to replace deprecated Text concatenation with interpolation.

For a contact without a last name, the first name supplies its sorting key and section. Nonalphabetic initials use `#`; accented Latin initials use their base letter. Favorites remain in the full alphabetical directory as well as the top favorites section. Editing the five specified fields preserves birthday and favorite values. Birthdays use a fixed Gregorian, UTC parsing calendar so they do not shift with time zones.

## Verification

The isolated simulator verification configuration passed **52 assertions**: 30 on the write run and 22 after terminating and relaunching the process. It checks bundled counts and IDs, sorting, all search fields, whitespace names, first/last-only names, accent matching, missing birthdays, add/edit/delete, favorite on/off, saving an empty directory, save failures, and persistence across process relaunch. These checks use a separate temporary repository, which is removed afterward; the shipped directory retains the original 320 contacts.

Reports: `build/verification-write.txt` and `build/verification-read.txt`.
Screenshots in `build/` cover the main directory, details, existing-contact editing, a new-contact phone keyboard, phone search, no results, the Z section jump, dark mode, and the largest text setting.

For rerunning the assertions, build with `CONTACTS_TESTING=1 SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run`, then launch with `--verify-write`, terminate/relaunch with `--verify-read`, and inspect the reports written to the app's Documents directory. The same configuration supports screenshot routes (`--detail ID`, `--edit`, `--editor`, `--keyboard`, `--search QUERY`, `--index-z`). These routes and the test suite are excluded from the final normal binary.

The final bundle was checked for its exact name/ID, an unchanged 320-contact seed, and exclusion of test routes. Its running process was verified on the designated simulator.

## Limits and what would help

No specification features remain unfinished. Calling and composing email depend on an installed handler, and cannot be fully exercised on this simulator; unavailable handlers get a clear alert.

Direct desktop UI automation could not access Device Hub, and no Simulator app surface was available through that tool. UI verification therefore used simctl-driven view setups, real simulator screenshots, and in-app assertions rather than automated touch input. A simulator-scoped tap/type/accessibility tool would have made end-to-end gesture checks faster and more thorough.
