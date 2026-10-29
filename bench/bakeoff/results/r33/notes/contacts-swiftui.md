# Contacts build notes

Completed and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.bdcf881`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is open on its directory. No other simulator was used. The original 320 contacts and 31 favorites remain after removing test contacts. The simulator is back in light mode at its original `large` text size; the temporary UI-test runner was removed.

## What was hard

The skeleton had no Xcode project or interaction-test runner, so I created a small independent UI-test target that controls the preinstalled app by bundle ID. Simulator UI details required care: search uses a Close control on this runtime, and typing over a long search query needs the native clear button rather than deleting from an arbitrary cursor position.

Visual review at the largest accessibility text size caught issues that basic interaction checks missed. The Favorites heading needed more horizontal space, empty-state content needed scrolling, and focused form fields needed explicit scrolling after the keyboard changed the safe area. The phone pad has a Done accessory; other fields use their native Next or Done keys.

Persistence distinguishes a missing save file from a deliberately empty directory. Writes are atomic, failed writes leave the current contacts unchanged, and damaged saved data is protected from replacement. Birthdays use a fixed time zone when parsing and displaying date-only values.

## Lookups and assumptions

I checked the local SwiftUI SDK interface for the native `sectionIndexLabel` and `listSectionIndexVisibility` APIs, and used `simctl` help for accessibility text-size settings. No web research or third-party dependencies were needed.

Favorites are a horizontally scrolling section, sorted by last name, and also remain in the complete alphabetical directory. Names without a last name go under `#`. Search ignores case and accents, accepts reversed name terms, and matches phone digits without punctuation. Editing exposes the five fields requested and preserves existing birthdays and favorite status.

## Verification

Model checks passed for the seed data, sorting, search normalization, whitespace-only name rejection, first-only and last-only names, optional company and birthday fields, leap-day birthdays, favorite changes, relaunch persistence, deletion, an empty saved directory, and damaged storage.

Simulator UI checks passed for section-index jumps; name, company, phone and email search; clearing search; known and unknown birthdays; phone and email actions; creation and editing; favorites and relaunch persistence; list and detail deletion; cancellation and discarding edits. Dark-mode screenshots and keyboard visibility checks passed at `accessibility-extra-extra-extra-large`, including phone and email input and empty search results. The final save-and-relaunch regression also passed. Logs, result bundles, and screenshots are under `build/`; `Tests/run.sh` makes the checks repeatable.

## Remaining limitations and faster tooling

No specification features remain unfinished. The simulator cannot actually place calls or send mail through a configured account; the app opens the appropriate system URL and offers a clear explanation and copy alternative when no handler is available.

A supplied simulator interaction-test target or a built-in tap/type/accessibility-inspection CLI would have saved the most time. A standard asset-catalog compilation step in the skeleton would also have simplified adding the app icon.
