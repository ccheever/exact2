# Inbox — build notes

Finished and installed as `dev.bakeoff.inbox.bf47fda` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The app is open on the normal Inbox, with the original 320 messages and 109 unread messages. No other simulator was used.

## Implementation

- UIKit app and scene delegates, navigation controllers, native tab bar, self-sizing table cells, search, segmented filters, context menus, swipe actions, and pull to refresh.
- Swipe left to archive or flag; swipe right to toggle read state. Archive supports moving messages back to Inbox. The reader has archive/restore, flag, and read controls.
- Bundled `Resources/mail.json` is an exact copy of the supplied file. State and locally generated mail are saved atomically in Application Support. Failed saves roll back the change and show an error.
- Dynamic Type, dark appearance, VoiceOver labels and custom actions, empty states, unread counts, and a custom app icon are included.

## Decisions and assumptions

The dataset contains preview text but no separate body. The reader shows the supplied preview as the message content. Filters apply to the current mailbox, and the Inbox badge excludes archived mail. Each refresh creates two unread messages, with unique IDs and timestamps newer than existing mail; refreshed messages persist too. No external research was needed.

## What was hard

The largest accessibility size exposed an accidental second round of font scaling, which was corrected. Row alignment also needed an explicit avatar-column width. Filtering while opening a message required keeping navigation tied to the message ID, so removing the read message from the Unread list does not change the selected message. Simulator access required running the build and simctl commands outside the filesystem sandbox.

## Validation

24 automated simulator checks passed, including dataset decoding, ordering, filtering, opening/marking read, reader controls, actual swipe action handlers, archive membership, saving/restoring state, empty search, repeated refresh, and unique IDs. A separate terminate-and-relaunch check passed for archived, flagged, unread, and refreshed-message persistence. QA stores are isolated from the normal mailbox.

Screenshots were inspected for light and dark modes, the largest accessibility size, reader, search with keyboard, Archive, and refreshed Inbox. The supplied simctl toolkit has no touch-input command; interactions were exercised through UIKit controls, controller callbacks, and swipe action handlers rather than synthesized finger gestures.

Reports: `build/QA.txt`, `build/QA-persistence.txt`. Screenshots include `build/final-inbox.png`, `build/inbox-dark-unread.png`, `build/inbox-largest-fixed.png`, `build/reader-largest.png`, `build/search-keyboard.png`, `build/archive.png`, and `build/refreshed.png`.

## Unfinished work and tooling

No specification features remain unfinished. A ready-made simulator input driver and an existing UI-test target would have made gesture and navigation verification faster. A message body field would have removed the content assumption.

Rebuild with:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```
