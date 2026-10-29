# Inbox build notes

Finished, built, installed, and launched as `dev.bakeoff.inbox.b5b4538` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final installation starts with the original 320 messages (109 unread and 22 flagged). All requested features are implemented.

## Implementation

- Programmatic UIKit app delegate, scene delegate, navigation, self-sizing mail rows, native swipe actions, and a scrolling message reader.
- All original data is shipped unchanged as `Resources/mail.json` and copied into the app bundle.
- Read, flagged, archived, and locally generated mail are saved together in an atomic JSON file in Application Support. Failed writes roll back the change and show an error. Invalid saved JSON falls back to the bundled mailbox with a notice.
- Refresh adds exactly two unread messages with unique IDs and dates newer than the current newest message. It preserves existing mail and state.
- Also includes search, context menus, accessible action labels, archive restoration, six-second archive undo, empty states, and an envelope app icon.

## What was hard

Self-sizing table headers and empty states needed care so Dynamic Type could expand everything without overlapping content. At accessibility sizes, dates move below senders and the filter bar scrolls horizontally. Keyboard layout guides keep search results above the keyboard. Custom weighted fonts use an unscaled base font to avoid scaling twice at the largest size. Dark mode uses a lighter accent with dark text on selected filters for contrast.

The starter has no Xcode project. I created a small project under `Tests/` to run simulator UI tests while keeping `build.sh` as the final build/install path. Simulator services and SDK execution needed execution outside the shell sandbox.

## Lookups and decisions

Read the starter README and build script, inspected the data schema, used compiler diagnostics for UIKit selectors, and checked `simctl ui` help for appearance and content-size names. No web services or third-party dependencies were needed.

The dataset provides previews but no separate message body, sender address, or recipient address. The reader displays the supplied preview as the message text and identifies the recipient as “You.” Refresh content is made up locally, as requested. Archive supports moving messages back to Inbox.

The simulator's native search dismissal button is labelled “Close,” rather than “Cancel.” UIKit can retain a recycled table cell in XCTest's accessibility snapshot after the final row is removed; tests verify the visible empty state and that the old row cannot be tapped. Screenshot checks also allow navigation rendering to settle.

## Verification

- Storage checks passed: bundled counts and IDs, newest-first ordering, read/flag/archive persistence, archive restoration, repeated refresh, generated IDs, write-error rollback, and corrupt-save recovery.
- The full simulator UI flow passed: filters, flag persistence across relaunch, opening unread mail, both swipe directions, archive persistence, restore, search with the keyboard, exactly two refreshed messages, ordering, and refresh persistence after relaunch.
- Separate reader-control and undo checks passed: unflagging, marking unread, filter membership, undoing archive, and the empty Archive screen.
- Dark mode at `accessibility-extra-extra-extra-large` passed, with screenshots of Inbox, the reader, the scrolled message body, and empty Archive.
- The final `build.sh --run` succeeded. Validated the final plist, exact bundled JSON copy, and icon dimensions, then captured `build/inbox-final.png`.

The simulator was returned to light appearance and the normal Large text size. The test runner and test mailbox changes were removed before the final install. No other simulator was booted or used.

## Unfinished work

None of the requested features are unfinished.

## What would make this faster

A ready-made UIKit test target and a small simulator interaction/screenshot harness would avoid the initial project setup and test-driver adaptation. A full message body and email-address fields would remove the need to interpret previews as reader content.
