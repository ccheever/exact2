# Contacts — build notes

Completed and installed as `dev.bakeoff.contacts.b9f79e1` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is left open on its directory in light mode at the standard Large text size. No other simulator was used.

## Implementation

- Bundles the original 320 contacts unchanged in `Resources/contacts.json`.
- Last-name sorting and alphabetical sections, with a tappable and draggable section index and a favorites shortcut.
- Favorites appear in a horizontal section above the complete directory.
- Live search covers name, company, email, and phone; it ignores case and accents and supports phone searches without formatting.
- Details include company and known birthdays. Phone/email open the appropriate system URL, with useful feedback when the simulator cannot handle it. Touch and hold to copy either value.
- Add/edit forms require a nonblank first or last name, use appropriate keyboards, and protect unsaved edits. Both list and detail deletion ask for confirmation.
- Changes are written atomically to the app's Documents directory. A failed write leaves the in-memory contacts unchanged. An intentionally empty saved directory remains empty on relaunch.
- Native SwiftUI navigation, forms, search, sheets, alerts, dark mode, Dynamic Type, accessibility labels, and a custom app icon.

## Verification

The model regression executable passed seed loading, sorting, all search fields, phone normalization, accents, whitespace validation, first-name-only contacts, add/edit/favorite/delete persistence, an empty saved directory, and write-failure rollback.

Simulator XCTest checks passed:

- Add, favorite/unfavorite, edit, relaunch, and delete from details; deletion survives relaunch.
- Alphabet jumps to Z and back to A, phone search, birthday and actionable detail fields, clearing search, last-name-only creation, and swipe deletion.
- Directory, details, and editor in normal light mode and dark mode at accessibility-extra-extra-extra-large, with screenshots reviewed. Favorite cards widen at accessibility sizes; email addresses wrap without adding visible hyphens.

Temporary test contacts were deleted. The directory is back to 320 contacts and 31 favorites. The temporary UI test runner was uninstalled. Results and screenshots are in `build/`; the final normal-size screenshot is `build/final.png`.

Rebuild/install/launch:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

Model checks:

```sh
xcrun swiftc -parse-as-library Sources/Contact.swift Tests/ContactTests.swift -o build/contact-tests
./build/contact-tests
```

UI checks: `Tests/build-ui-tests.sh` builds a lightweight XCTest runner without requiring an Xcode project and targets only the designated simulator.

## Hard parts, lookups, and decisions

The restricted shell initially blocked Swift's macro compiler and simulator services; building and testing required the permitted toolchain access outside the shell sandbox. Desktop UI control was unavailable, so simulator interaction used XCTest and simctl screenshots. The direct-swiftc starter had no test target, so assembling a standalone UI test runner took extra work. Its executable metadata needed to be filled in from the Xcode template, and this iOS version labels search dismissal “Close” rather than “Cancel.”

I used local Xcode tool help and SDK paths rather than web research. Layout at the largest accessibility size needed wider favorite cards, fewer decorative elements, full-height wrapping detail values, and explicit email break opportunities.

The spec did not define grouping when a last name is absent: these contacts group and sort by first name. Favorites remain in the complete alphabetical directory as well as in their shortcut section. Birthday is displayed and preserved when editing; the editor contains the five fields specified for add/edit.

Nothing in the specification remains unfinished. Actual telephone calls and outgoing mail cannot be fully exercised on a simulator without the corresponding services; their controls and URL handling are implemented.

A preconfigured simulator UI test target, working desktop simulator control, and compiler/simulator access enabled at the outset would have made this faster.
