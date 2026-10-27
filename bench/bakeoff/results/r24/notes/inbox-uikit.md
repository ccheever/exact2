# Inbox — build notes

Finished and installed as `dev.bakeoff.inbox.b1aba1a` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The normal app is open with the original mailbox; diagnostic runs use a separate disposable store.

## Implemented

- Programmatic UIKit app and scene delegates, navigation controllers, and Inbox / Archive tabs.
- All 320 original messages bundled unchanged as `Resources/mail.json`, newest first.
- Sender initials, subject, two-line preview, local date/time, unread dots, and per-mailbox unread counts.
- Leading swipe for read/unread; trailing swipe for archive/restore and flag/unflag. Flags are also available on rows, in the reader, and through context menus and VoiceOver actions.
- All, Unread, and Flagged filters in both mailboxes, with appropriate empty states.
- Opening marks a message read. The reader shows the supplied preview as its body, because the dataset contains no separate body field. Reader actions include flagging, changing read state, and moving between mailboxes.
- Pull-to-refresh and the refresh button add exactly two locally generated unread messages. Repeated triggers are guarded, IDs stay unique, and new messages sort above existing mail.
- Atomic local persistence of messages and read, flagged, and archived state, including generated mail.
- Adaptive colors, Dynamic Type through the largest accessibility size, a larger filter menu at accessibility sizes, and accessibility actions. Original message content is selectable in the reader.
- Custom app icon generated with AppKit vector drawing; no remote assets or network functionality.

## Validation

Built and installed with the provided build script. The simulator self-test passed 29 checks covering seed counts, sorting, filters, reading, state changes, archiving/restoring, persistence, repeated refreshes, empty mailboxes, and the pull-to-refresh target and spinner lifecycle. A separate terminate-and-relaunch test confirmed 322 messages and the saved unread, flagged, and archived states survived a full process restart.

Visually checked simulator screenshots of light and dark lists, the reader, populated and empty Archive screens, and the largest accessibility text size. Screenshots are in `build/`. Tests drive UIKit targets programmatically; touchscreen gesture automation was not supplied by the toolkit. Swipe configurations and their corresponding mutations are checked programmatically.

To repeat the disposable checks after building:

```sh
xcrun simctl launch --terminate-running-process --console EF198926-1958-489D-A7A4-9196E454DD3A dev.bakeoff.inbox.b1aba1a --self-test
```

Then launch without arguments to return to normal mail. `--ui-test` supports `reader`, `unread`, `flagged`, `archive`, `empty`, and `refresh` screenshot scenarios.

## Hard parts and decisions

Refreshing exposed a subtle ISO-8601 persistence precision issue: fractional seconds disappear with the standard encoder. Generated timestamps now use whole seconds, preserving exact reload equality and ordering. Keeping filtered lists consistent when a message becomes read, unflagged, or archived required updates to both mailboxes and counts from one store notification.

The largest text size benefits from vertically arranged sender/date information and a menu replacing the compact segmented filter. The reader scrolls with no fixed text heights. Dates use the device locale and time zone; today shows a time, yesterday is labeled, and older messages show a date.

Lookups were limited to the provided README/build script, data inspection, and `simctl ui` command help. Simulator access needed the toolkit command to run outside the filesystem sandbox. No web research was necessary.

Nothing in the requested feature set remains unfinished. A supplied simulator touch/gesture driver and a starter UIKit lifecycle/icon setup would have made the build and end-to-end interaction checks faster.
