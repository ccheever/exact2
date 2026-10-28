# Contacts — build notes

Completed and installed as **Contacts** (`dev.bakeoff.contacts.b8c2194`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final app is launched at its directory screen. No other simulator was used.

## Implemented

- All 320 supplied contacts are bundled in `Resources/contacts.json` unchanged.
- Last-name ordering, alphabetical sections, and a tappable/draggable alphabet index.
- Immediate search across names, companies, email addresses, and phone numbers, including phone formatting variations.
- A favorites section above the full alphabetical directory; favorite state can be changed on details or from a row context menu.
- Details with tappable phone/email, copy context menus, and known birthdays.
- Add/edit sheets with appropriate keyboards, a keyboard dismissal button, name validation, and discard confirmation.
- Confirmed deletion from list swipe actions/context menus and from details.
- Atomic local JSON persistence. Failed saves leave the current directory intact and report an error. An intentionally empty directory remains empty after relaunch.
- Light/dark appearance, Dynamic Type through the largest accessibility size, VoiceOver labels, and an app icon.

## Testing

`build/qa/results.txt` records **35/35 passing simulator data checks**. These cover the supplied dataset, search, ordering, optional birthdays, invalid/whitespace names, single-name contacts, trimming, accented initials, additions, edits, favorites, deletions, empty storage, and write failures.

`build/qa/persistence.txt` records successful create, edit, delete, and verify phases across four separate app launches. The test contact was removed afterward. Final verification confirmed the installed app has all original 320 contacts and 31 favorites, with every original field preserved.

Screenshots in `build/qa/` were visually inspected for the directory, details, editor, phone keyboard, alphabet jump, search results, no results, dark mode, and the largest accessibility text size. Compile-time QA navigation was used for screen inspection; this was not touch-driven XCTest automation. The final binary excludes those QA routes. `Tests/run_qa.sh` reproduces the data/relaunch checks and reinstalls the normal app afterward.

The final build command was:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```

## Hard parts and choices

SwiftUI does not provide a ready-made alphabet index for this list layout. A small index control drives `ScrollViewReader`, supports dragging, and exposes individual letters to accessibility. Large accessibility fonts also needed wider favorites cards and multiline names, while list rows drop decorative avatars to leave more room for text.

Favorites appear in a horizontal section and remain in their normal alphabetical position. Contacts without a last name are sorted/sectioned by first name; nonletter initials use `#`. Editing preserves birthdays and favorite state. Birthday editing was not added because the requested editor fields were first name, last name, company, phone, and email.

Phone/email actions use system URL handling. Calling or email composition depends on device support; unavailable services produce a helpful message and numbers/addresses can be copied. Real calls and email delivery cannot be validated on this simulator.

## Lookups and improvements to the workflow

I read the supplied README, build script, and JSON schema and used compiler feedback and simulator screenshots; no external web research or third-party libraries were needed. Xcode compiler plugins and CoreSimulator required execution outside the default shell sandbox.

A supplied simulator touch/typing automation helper would have made interaction checks faster and more comprehensive. A standard Xcode test target would also simplify repeatable UI regression coverage. No requested app feature remains unfinished; the testing limitations are described above.
