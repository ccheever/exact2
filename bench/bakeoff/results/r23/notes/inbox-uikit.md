# Inbox build notes

Completed all requested features in programmatic UIKit. The finished app is built, installed, and launched on `EF198926-1958-489D-A7A4-9196E454DD3A`, with bundle ID `dev.bakeoff.inbox.be3c520`. No other simulator was used.

`Resources/mail.json` ships all 320 messages unchanged; its SHA-256 matches the supplied data and the installed app bundle. Changes and locally generated messages are written atomically to Application Support. QA uses separate cache files, so the normal inbox starts with the supplied data.

## Decisions and assumptions

- The dataset has no separate body field, so the reader displays the complete supplied preview as the message body.
- Unread counts describe each mailbox; the Inbox tab badge excludes archived mail.
- Left swipe offers Archive and Flag; right swipe toggles read status. The reader and VoiceOver actions also expose these operations. Archive supports restoring messages to Inbox.
- Added local search, context menus, and a confirmed Mark All as Read action.
- Refresh creates exactly two unread messages, with unique IDs and timestamps newer than existing mail, even if the device clock moves backward.

## What was hard

The simulator's current UIKit places the large navigation title inside the scrolling content. An opaque navigation appearance covered it; native transparent scroll-edge styling fixed that. At the largest accessibility size, empty states needed to be scrollable table content rather than centered background views. New message timestamps also needed whole-second precision to match ISO-8601 persistence. A final layout assertion needed a pixel tolerance for floating-point rounding.

## Lookups and verification

Used the skeleton README, local compiler diagnostics, UIKit layout inspection, and `simctl` help for appearance, content size, screenshots, and launch options. No web research or external dependencies were needed.

Simulator checks passed: 25 data/model checks, 23 UIKit checks at standard size, 23 UIKit checks at the largest accessibility size, and two checks across separate process launches. Checks cover filters, reading, flagging, archive/restore, search, refresh, counts, and persistence. UIKit checks exercise table delegates and control target/actions and inspect native swipe configurations; they do not synthesize finger gestures. Reviewed screenshots in light/dark mode, maximum text size, scrolled empty states, the reader, and search with the keyboard visible. Reports and screenshots are in `QA/`.

Nothing remains unfinished. A UIKit lifecycle skeleton and a supported simulator touch-injection tool would have made setup and gesture verification faster.

Rebuild and launch:

```sh
SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run
```
