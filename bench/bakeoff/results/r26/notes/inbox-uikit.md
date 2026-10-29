# Inbox build notes

Finished and installed as `dev.bakeoff.inbox.bd563e1` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The app is left open on Inbox, with normal text size and light appearance. No other simulator was used.

Implemented the entire specification in programmatic UIKit: newest-first messages, unread indicators and counts, read/unread and archive swipes, Archive with restore, flag buttons and actions, unread/flagged filters, reader, pull-to-refresh adding exactly two local messages, and atomic persistence. Search, empty states, VoiceOver actions, dark mode, and accessibility layouts are included. All 320 source messages are bundled unchanged in `Resources/mail.json`.

## Decisions and difficult parts

- The source has no full message body or recipient address. The reader displays the supplied preview as the available message text and uses “To: me.” No sender addresses or additional original-message content were invented.
- Refreshed messages have local sample content, unique increasing IDs, and timestamps newer than the existing newest message, even if the device clock is earlier. They persist alongside all message states.
- All changes save immediately to Application Support using atomic file replacement. Failed writes leave the previous state intact and show an error.
- UIKit navigation and toolbar behavior on the simulator required visual iteration: the reader hides the tab bar so its actions have their own space. Search appears on demand, and the list follows the keyboard layout guide.
- At the largest text size, sender and subject wrap, avatars give way to more text space, unread state gets an explicit text label, and the header stays compact. Previews remain limited to two lines as specified.
- No web lookup or external dependencies were needed. The provided README and build script established the compiler, resource, and simulator workflow. A small AppKit script generates the app icon locally.

## Verification

- Built and installed with `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`.
- All 24 simulator-hosted functional checks passed. They exercise UIKit action handlers, selection, filters, search, persistence reloads, repeated refreshes, and the refresh control, including overlapping refresh requests. See `Verification/self-test-results.txt`.
- A separate isolated mailbox passed actual process termination and relaunch with read, flagged, archived, and refreshed messages preserved. See `Verification/persistence-results.txt`.
- Visually inspected screenshots of Inbox, unread filtering, the Archive empty state, native search and keyboard, and the reader, including dark appearance and the largest accessibility text size. Screenshots are in `Verification/`.
- Tests use isolated files and preserve the app’s original 320-message mailbox. The toolkit provided no external gesture driver; swipe behavior was checked through its UIKit action handlers rather than synthesized physical swipes.

## Remaining work and what would help

No specification features remain unfinished. Full message bodies would make the reader more realistic. A supplied simulator touch driver and a starter UIKit lifecycle would have made interaction verification and initial setup faster.
