# Inbox — build notes

Completed and installed as **Inbox** (`dev.bakeoff.inbox.bfc96d9`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`.

## Implementation

- Programmatic UIKit app delegate, scene delegate, navigation controllers, self-sizing table cells, native search, menus, swipe actions, refresh control, and reader toolbar. No storyboards, SwiftUI, networking, or sending.
- The original 320-message `data/mail.json` is shipped unchanged as `Resources/mail.json`. The finished bundle was checked against the original file byte for byte.
- Messages sort newest first, with stable ID ordering for tied timestamps. Read, flagged, archive, and locally refreshed messages save together in an atomic JSON file in Application Support. Failed saves leave the displayed state unchanged.
- Leading swipe toggles read/unread; trailing swipe offers Archive and Flag. Archive supports restoring to Inbox, and moving messages offers Undo. Reader actions and VoiceOver custom actions expose the same operations.
- Search covers sender, subject, and preview. All / Unread / Flagged filters apply to the current mailbox. The unread total counts the current mailbox, regardless of search or filter.
- Dynamic colors support dark mode. Accessibility text sizes stack sender/date and mailbox counts, wrap subjects, and replace the segmented filter with a native menu button. The reader and empty states scroll.
- Added a locally drawn app icon.

## Decisions and assumptions

The data contains only a preview, with no separate message body or email addresses. The reader therefore displays the provided preview as the message text, without inventing additional content. Refresh is the only source of invented mail: every pull adds exactly two unread, unflagged messages with unique IDs and timestamps newer than all existing messages. Those additions also survive relaunch.

## What was hard

UIKit navigation/search behavior on this simulator’s iOS version required visual verification: automatic search placement moved the field to the bottom, and large navigation titles did not render reliably. A compact native navigation title and explicit stacked search placement produce a consistent result.

Dynamic Type needed careful baseline font scaling to avoid scaling weighted fonts twice. Self-sizing table headers, hidden filter controls, large-text menus, and scrollable empty states also required layout adjustments.

The UI automation initially confused the Unread filter with the Unread swipe action. It now selects the action by its position below the filter. The system search dismissal control is named Close on this runtime, so the test accommodates both Close and Cancel.

## Validation

- Built, installed, and launched with the supplied `build.sh --run` command and assigned simulator UDID; final build completed without compiler warnings.
- Foundation store checks passed: all 320 records, initial counts (109 unread, 22 flagged), sorting, search, filters, state changes, archive/restore, repeated refresh, unique IDs, persistence, change notifications, and failed atomic saves.
- Full XCTest UI flow passed: both swipe directions, opening marks read, flag/unflag, filters, archive/restore, Undo, empty states, pull-to-refresh, keyboard/search dismissal, and state plus refreshed-mail persistence across process termination/relaunch.
- Largest accessibility text UI test passed, including filter menu selection, reading, flagging, archive, Undo, and empty Archive. Rechecked after the final adaptive layout adjustment.
- Reviewed light, dark, and largest-text screenshots. Screenshots are in `Tests/Screenshots/`; passing logs are `Tests/final-test.log` and `Tests/accessibility-test.log`.
- Tests use an isolated state file. The final installed app opens on the original 320-message inbox. The test runner was removed from the simulator.

To repeat verification, run `Tests/run.sh`. It targets only the assigned simulator. Reinstall the normal app afterward with `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`.

## Lookups, unfinished work, and speed

Read the starter README/build script and local command help for simulator IO and XCTest screenshot export. No web research or external dependencies were needed. No specification features remain unfinished.

A supplied UIKit starter with a scene manifest and a ready-to-run simulator UI test target would have made this faster. A body field in the source data would allow a richer reader without guessing content.
