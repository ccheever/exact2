# Inbox build notes

Finished and installed as `dev.bakeoff.inbox.b064028` on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The app is left running on its normal Inbox screen, in light mode at the standard text size.

## What shipped

Programmatic UIKit app and scene delegates, native navigation, searchable newest-first Inbox, two-line previews, unread indicators and counts, leading read/unread swipes, trailing archive swipes, inline flags, unread/flagged filters, a message reader, and a searchable Archive with move-back-to-Inbox support. Archiving offers Undo. Pull-to-refresh and the refresh toolbar button each add two local messages. Read, flagged, archived, and newly received messages are saved atomically in Application Support.

`Resources/mail.json` and the copy in the built app are byte-for-byte copies of `data/mail.json`, containing all 320 messages. The app includes a custom envelope icon, adaptive light/dark colors, Dynamic Type, VoiceOver summaries and actions, and scrollable empty states.

## Hard parts and decisions

- The largest accessibility text sizes truncate a three-segment control. At those sizes the filters become a full-width native menu; rows hide decorative avatars and place dates below senders. The bottom count has a bounded width.
- iOS 26 large-title transitions between searchable mailboxes produced a missing Archive title. Archive now uses a compact native navigation title, set before presentation.
- Deferred store notifications initially cleared the refresh confirmation immediately. The confirmation now remains visible briefly while keeping its unread count current.
- The input supplies previews but no separate message body, so the reader displays the complete supplied preview as its body. No sender addresses or additional original-message content were invented.
- Refresh messages use local templates, unique increasing IDs, and timestamps newer than the newest existing message, including after rapid repeated refreshes.

## Verification

Built and launched using the supplied `build.sh`. Thirty simulator controller/storage checks passed with zero failures, including filtering, search, reader actions, archive/undo/restore, repeated refresh, concurrent-refresh protection, and persistence. A separate terminate-and-relaunch check verified read, flag, archive, and new-message state across processes. Tests restore the original inbox state and use isolated files for generated mail.

Visually inspected simulator screenshots of the inbox, flagged list, reader, Archive, search keyboard, dark mode, and largest accessibility text. Reports and screenshots are in `build/qa/`. The opt-in `--inbox-test` launch argument runs the checks; `--persistence-write` followed by a terminate and `--persistence-read` runs the separate relaunch check. These hooks do not run on normal launches. Touch swipes themselves were not synthesized: their native configurations and resulting state transitions were checked programmatically.

## Lookups, unfinished work, and speedups

Used the starter README, installed UIKit SDK/compiler, and `simctl`; no web research or external dependencies were needed. No specified feature remains unfinished. A ready-made simulator touch driver and an initial accessibility screenshot matrix would have made gesture verification and layout iteration faster.
