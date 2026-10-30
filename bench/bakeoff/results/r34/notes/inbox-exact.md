# Inbox build notes

Built with Exact and installed and launched as **Inbox** (`dev.bakeoff.inbox.b280418`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. No other simulator was used.

## Implemented

- All 320 messages from `data/mail.json` are embedded through the data-module import; the original data file remains unchanged.
- Newest-first inbox, sender, subject, preview, local date/time, unread dots, and unread count.
- Native left swipe: Archive and Flag/Unflag. Native right swipe: Mark Read/Unread. Full swipes perform archive/read actions.
- Archive screen, message reader, restore to inbox, unread/flagged filters, and case-insensitive search.
- Opening a message marks it read. Detail controls and long-press menus expose the message actions.
- Pull to refresh adds exactly two local unread messages. Each refresh uses unique IDs and sorts ahead of existing mail even if the device clock moves backwards.
- Atomic local-file persistence keeps read, flagged, archived, and newly added messages across relaunches. Writes use one queue.
- Native navigation, search and keyboard, semantic colors, dark mode, empty states, and accessibility text scaling.

## Validation

- `bun exact.mjs verify`: all 9 feature tests passed. The web layout report has findings around hidden swipe actions in wide split views; iPhone is the delivery target.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: **clean**, 9 tests passed, 0 failed. Initial screen, 9 test ends, and relaunch passed light/dark checks. All 11 checked screens fit at accessibility-extra-extra-extra-large (53-point body).
- The iOS scrolling check found nothing for the app to fix. It reported toolkit-level rubber-band and status-bar scroll-to-top differences; performance timing was not judged because the host machine was heavily loaded.
- Real simulator touches independently verified pull-to-refresh (320 → 322 messages), full leading swipe to mark read (111 → 110 unread), full trailing swipe to archive (322 → 321 inbox messages), and Archive navigation (1 archived message).
- Final `bun exact.mjs ios --run` succeeded and left the standalone embedded build installed and running.
- Screenshots and verification output are under `.exact/verify/`; gesture screenshots are under `.exact/`.

## What was hard / looked up

- Looked up Exact's list-detail pattern, native swipe action structure, refresh handlers, file grants, date formatting, icon tokens, native menus, and iOS touch-driver syntax.
- Initial toolkit compilation took a few minutes and required access to the toolkit's build cache outside this app directory.
- A segmented filter clipped in large-text web checks. Replaced it with a native toolbar menu and a subtitle identifying the active filter.
- Arbitrary SF Symbols require portable icon tokens to avoid warnings for the generated web/Linux targets. Added small filled SVG fallbacks while retaining native SF Symbols on iPhone.
- Physical-touch mode cannot directly tap an authored swipe-action identifier. Verified those actions with real full swipes instead.

## Decisions and limits

- The supplied data has a preview but no full message body or email address. The reader shows the complete supplied preview as its body and identifies the recipient as “You.” No additional original-message content was invented.
- Preview text can expand at accessibility sizes rather than clipping useful content.
- Local refresh messages are deliberately fictional and cycle through several pairs; IDs and timestamps remain new for every refresh.
- All specified iPhone functionality is complete. No sending or network mail service was added.

## What would have made this faster

A mail-specific Exact starter combining bidirectional native swipes, bundled seed data, persistent state, and refresh would have helped. Better verifier recognition of intentionally hidden swipe controls in web split views would also reduce noise. Native swipe-action tapping support in physical-touch mode would simplify gesture tests.
