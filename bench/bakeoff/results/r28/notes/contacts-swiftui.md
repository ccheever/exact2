# Contacts — build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.bf566d3`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final normal build is running on the directory screen; simulator appearance and text size were restored to light and standard Large.

## Implementation

- Ships the original 320-contact JSON unchanged in `Resources/contacts.json`.
- SwiftUI navigation, alphabetical sections, native section index, live search, horizontal Favorites section, detail screens, add/edit forms, swipe/context-menu deletion, and confirmed deletion from details.
- Favorites also remain in the alphabetical list so every contact can be found by surname. The horizontal section keeps 31 starting favorites from pushing the directory far down the screen.
- Names are required after trimming whitespace. First-name-only contacts sort under their first name; non-Latin/nonletter initials use `#`. Search ignores case and accents and supports phone digits without formatting.
- Known birthdays are displayed and preserved when editing. The editor exposes the five fields named in the specification.
- Every change writes an atomic JSON snapshot to the app's Application Support directory before updating the visible data. An empty saved directory stays empty after relaunch. Load failures do not silently replace saved contacts with the seed; saving errors are reported.
- Native Dynamic Type, adaptive light/dark colors, accessible controls, scrolling forms, appropriate keyboards, keyboard Done, and a discard-changes confirmation. Includes a locally rendered address-book icon.

## What was hard / looked up

The main layout challenge was combining a usable section index with 31 favorites while supporting the largest accessibility text size. I checked the installed SwiftUI SDK interface for `sectionIndexLabel` and `listSectionIndexVisibility`, then used the native implementation. Simulator screenshots exposed oversized quick-action buttons under an initial adaptive layout; explicit compact/stacked layouts fixed that. Accessibility screenshots also led to wider favorite cards so surnames wrap sensibly.

No web research or external assets were needed. I chose the favorite-strip presentation, first-name fallback sorting, local JSON storage, and read-only preservation of birthdays where the specification left those choices open.

## Verification

- Built and installed with `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run`.
- `Tests/ModelTests.swift`: checked all 320 records against name, reversed-name, company, email and unformatted-phone search; sorted order; Unicode names; missing-name validation; phone/mail URLs; leap-day birthday formatting; saved/empty/corrupt-file behavior.
- A temporary simulator test build exercised adding, whitespace rejection, editing, birthday preservation, favorite/unfavorite, deletion and persistence across separate process launches. Verified that all original records were restored exactly after test cleanup.
- Reviewed simulator screenshots of directory, details, editor, search results, and the new-contact keyboard, including dark mode and the largest accessibility text size. Screenshots are under `build/screenshots/`.
- Test-only launch hooks are excluded from the installed final build. The test harness is in `Tests/SimulatorChecks.swift`; `Tests/build-test-app.sh` creates a separate temporary test build when needed. Store checks assume the untouched seed dataset.

## Limits and what would help

No specified feature remains unfinished. Actual telephone calls and delivery of email cannot be validated on this simulator; the app opens the system `tel:` and `mailto:` handlers, reports unavailable handlers, and offers copying through the information rows' context menus.

UI verification used screenshot review and test-only screen-state setup, rather than automated gesture taps or XCTest. A supplied simulator tap/accessibility driver would make testing navigation, section-index scrubbing, swipe actions, and keyboard interactions faster and more comprehensive. A ready-made app-icon asset setup would also shorten the final packaging work.
