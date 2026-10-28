# Contacts build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.bb7e52d`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. No other simulator was used. All specified features are implemented.

## Implementation decisions

- The bundled `Resources/contacts.json` is an exact copy of the 320-person source dataset. It seeds the directory until the first change. Subsequent changes are saved atomically in the app’s Documents directory; relaunches load that saved directory, including an intentionally empty directory.
- All contacts remain in the alphabetical list. The 31 starting favorites also appear in a horizontally scrolling section at the top, so they do not push the directory dozens of rows down.
- A contact with only a first name sorts and groups by that name. Accented initials are folded for section labels; nonletter initials use `#`. Name validation trims whitespace. Existing birthdays and favorite status are preserved during editing.
- Search covers name, company, email, and phone, including phone searches without formatting. Phone and email actions use the system’s `tel:` and `mailto:` handlers, with explanatory alerts when unavailable and long-press copying.

## What was hard and what I looked up

The starter has no Xcode project or UI automation harness. I added a standalone UI-testing project that drives the already installed app by bundle ID, while preserving the supplied SwiftUI command-line build. I checked the local SwiftUI SDK interface for the native `sectionIndexLabel` API; no web research was needed.

The largest accessibility text size exposed icon overlap and keyboard avoidance issues that ordinary interaction assertions missed. Decorative icons now have bounded sizes, the detail actions stack vertically, and editor scrolling waits until keyboard layout completes. The form reserves room for the floating Done button. A test now types in the email field and asserts that it stays clear of the keyboard and toolbar. An explicit discard alert also avoids platform differences in confirmation-sheet cancellation buttons.

The compiler’s macro process and CoreSimulator required execution outside the filesystem sandbox. App source, resources, test harness, and exported test artifacts were kept in this workspace.

## Verification

- Model checks passed for all 320 seed records, unique IDs, favorites, sorting, search, phone normalization, birthday formatting, whitespace validation, first-name-only sections, accented initials, and JSON round trips.
- Five UI scenarios passed on the assigned simulator: search/details/clear/empty results; add/edit/favorite/relaunch/delete; validation/cancellation/list deletion/birthday preservation; dark mode at the largest accessibility text size with keyboard input; and section-index jumps to Z and back to favorites.
- Temporary test contacts were removed. The installed app opens on the original 320-contact directory, with standard text size and light appearance restored.
- Final build and installation succeeded using `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run`.
- Repeat the checks against the starting directory with `Tests/run.sh`. Logs, result bundles, and screenshots are in `build/`; final accessibility screenshots use the `accessible-*-final.png` filenames.

## Remaining limitations and faster tooling

No specified work remains unfinished. Actual calls and email delivery require the corresponding system apps and cannot be exercised on this simulator.

A supplied UI-test runner and direct simulator interaction tooling would have shortened the build considerably. Screenshot inspection remains necessary alongside interaction tests, particularly for accessibility text and keyboard layout.
