# Contacts — build notes

Finished and installed as **Contacts**, bundle ID `dev.bakeoff.contacts.b32b57b`, on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final build is launched with normal text size and light appearance. No other simulator was used.

## Implementation

- Shipped the original 320-contact JSON unchanged in the application bundle.
- Built the directory, alphabet index (tap or drag), live search, favorites, details, add/edit forms, and deletion in SwiftUI.
- Search covers names in either order, company, email, and phone, including phone numbers without formatting.
- First-only and last-only contacts are valid; whitespace-only names are rejected. Contacts without a last name are sorted and indexed by their first name.
- Favorites appear in a horizontally scrolling section so all 31 starting favorites are available without pushing the alphabetical directory far down the screen. They also retain their place in the full directory.
- Optional birthdays are shown using locale-aware dates and preserved during edits. The requested editor fields do not change birthdays.
- Each mutation is saved atomically to the app’s Documents directory. Failed writes preserve the previous in-memory data. Empty saved directories do not reseed; unreadable saved files are not silently overwritten.
- Phone and email actions use system URLs, with a useful alert if the simulator cannot handle the action. Touch and hold either value to copy it.
- Added a custom app icon, adaptive colors, accessible row layouts, keyboard controls, and unsaved-edit protection.

## Verification

- Built, installed, and launched with the supplied `build.sh`.
- Standalone model checks passed for seed count, ordering, search, normalized phone matching, favorites, birthday preservation, first-/last-only names, save/reload, deletion, empty-store relaunch, corrupt-file protection, and rollback after a failed disk write.
- Six simulator UI tests passed with zero failures: temporary-data cleanup; directory/detail/editor navigation; add/edit/persistence/details deletion; favorite persistence; section index and search/clear/no-results; and last-name-only creation plus swipe deletion/cancellation.
- An additional simulator UI test passed in dark mode at `accessibility-extra-extra-extra-large`. Reviewed directory, details, and editor screenshots at normal and largest text sizes.
- Temporary test contacts were deleted, seed favorites restored, and the UI test runner uninstalled. The final directory has 320 contacts and 31 favorites.
- Rebuilt and reinstalled the finished app after testing. `build/final.png` captures the final launch. Test logs and result bundles are under `build/`. Repeat checks with `Tests/run.sh`.

## What was hard / looked up / decided

The starter uses bare `swiftc`, so a small Xcode UI-test project had to be assembled separately to automate simulator interactions. Compiler plugins, CoreSimulator, and Xcode’s screenshot exporter needed sandbox escalation. Consulted the provided README and local `simctl`/Xcode command help; no web research or external dependencies were needed for the app.

The simulator runtime presents some confirmation dialogs as anchored popovers, including while the search keyboard is open. Deletion now uses an explicit alert with Delete and Cancel. Tests also had to target alert buttons directly so XCTest would not automatically cancel an alert while trying to tap the underlying screen.

Design choices not prescribed by the specification: horizontal favorite cards; initials instead of contact photos; first-name fallback for missing last names; local-only storage; and confirmation before permanent deletion.

## Unfinished / limitations

No requested feature is unfinished. Actual calling and sending mail depend on a configured device; those services are unavailable in the simulator. There is no cloud sync or system Contacts integration, neither of which was requested.

## What would have made this faster

A preconfigured UI-test target and simulator interaction harness alongside the SwiftUI skeleton would have saved the most time. A documented runtime version and its confirmation-dialog behavior would also have reduced test-harness adjustments.
