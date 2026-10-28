# Inbox build notes

Implemented the specification with programmatic UIKit, an app delegate, and a scene delegate. Bundle ID: `dev.bakeoff.inbox.b631359`.

## Features

- All 320 supplied messages ship unchanged in `Resources/mail.json` and in the installed bundle.
- Newest-first inbox with sender, subject, up to two preview lines, local date/time, unread dots, flags, and unread counts.
- Swipe right to change read status; swipe left to archive or flag. Row flags, context menus, and VoiceOver custom actions provide additional access.
- Separate Archive screen, with restoration to Inbox through swipes or the reader.
- All / Unread / Flagged filters and local search across sender, subject, and preview.
- Opening a message marks it read. The reader offers read/unread, flag/unflag, and archive/restore controls.
- Pull-to-refresh and an accessible refresh button add exactly two locally generated unread messages. Repeated simultaneous refresh requests are coalesced.
- Messages and all state changes save atomically in the app's Documents directory and survive relaunch.
- System appearance, Dynamic Type, native search keyboard, scrollable empty states, and a custom app icon.

## What was hard

Native large-title navigation, a dynamically sized table header, filtering, and the largest accessibility text sizes needed careful coordination. The table header is measured explicitly to prevent the unread summary from being compressed away. At accessibility sizes, filters become a large native menu and sender/date information stacks vertically. Search results account for keyboard overlap.

## Lookups and assumptions

Used the supplied README/build script and local UIKit SDK headers to confirm contextual-action handlers and iOS 26 toolbar background APIs. No web research or external dependencies were needed. A selector named `move` conflicted with a UIKit responder method and was renamed.

The supplied data has previews but no complete bodies or email addresses. The reader displays all supplied message text without fabricating longer original emails. Refresh messages are made up locally, as requested. Inbox unread counts exclude archived messages; Archive has its own unread count. Filters and search operate within the current mailbox.

## Validation

Only simulator `498D6D1F-C21C-4B31-90F4-5D857216E256` was used. Built, installed, and launched with the supplied build script.

Simulator checks passed for source counts (320 total, 109 unread, 22 flagged), ordering, all filters, normalized search and empty results, actual contextual-action handlers for read/flag/archive/restore, the row flag button, reader navigation and actions, unique refresh IDs, and on-disk persistence. A separate two-process relaunch test verified read/flag/archive state and refreshed messages, then restored the original mailbox. The native refresh control test verified exactly two additions, overlapping-request protection, saving, and spinner completion.

Visually inspected standard light appearance, dark appearance at the largest accessibility text size, the reader at that size, and search with the keyboard visible. Screenshots are in `build/`. Bundled mail JSON checksums match the original file.

Simulator-only checks are in `Sources/SimulatorChecks.swift`; launch arguments `--inbox-checks`, `--refresh-check`, and the paired `--persistence-write` / `--persistence-read` exercise them. The paired persistence test must be run in order. Test changes were restored; the finished app is left showing the original Inbox.

## Unfinished work

None of the requested features remain unfinished.

## What would make this faster

A supplied simulator touch/gesture driver would allow end-to-end physical gesture testing directly. The available toolkit supports simulator lifecycle, screenshots, and appearance controls; interaction checks therefore invoke the real UIKit controls and action handlers from inside the simulator app. Sample full message bodies would also remove the need to interpret preview-only source content.
