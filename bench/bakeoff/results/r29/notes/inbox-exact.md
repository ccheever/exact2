# Inbox build notes

Finished and installed as **Inbox**, bundle ID `dev.bakeoff.inbox.b3dcbf9`, on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

## What shipped

- All 320 supplied messages, newest first, with unread markers, flags, dates, sender, subject, and preview.
- Native swipe actions in both directions: archive/restore, read/unread, and flag/unflag.
- Inbox and Archive navigation, a message reader, search, and All/Unread/Flagged filters.
- Pull-to-refresh adds two local unread messages. Receive Mail in the filter menu provides the same action.
- Atomic local storage preserves message states and newly received messages across launches.
- The original `data/mail.json` is imported into the data module and also shipped unchanged as `assets/mail.json`.

## Validation

- `bun exact.mjs test web`: **8 passed, 0 failed**.
- `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios`: **clean**, all **8 tests passed**.
- iPhone light and dark screens: no findings. Largest accessibility text size: nothing spills.
- Native scrolling audit: nothing for the app to fix. Host load prevented an absolute timing judgment; scrolling physics were checked.
- Real simulator touches confirmed full archive and mark-unread swipes. A real pull added exactly two messages, increasing the message count from 320 to 322 and unread count from 109 to 111.
- Final `bun exact.mjs ios` built and installed the standalone app on the assigned simulator.

## What was hard and what I looked up

Exact’s guide, list-detail starter, swipe vocabulary, filesystem grants, date formatting, icon tokens, native menus, and simulator-driving documentation supplied the implementation details. Native and web reload navigation differ: iOS returns to Inbox, so persistence tests explicitly reopen their target after relaunch.

The largest-text audit caught narrow status-icon columns and cramped segmented filters. The finished app uses a native filter menu and text-relative status columns. Previews naturally occupy up to two lines at ordinary iPhone sizes and expand at accessibility sizes.

Real swipe gestures needed start points inside the viewport. For pull-to-refresh, starting near the list’s top hit the overlaid navigation region; starting in visible list content worked. An early background build also needed rebuilding after bundled assets changed during compilation.

## Assumptions and unfinished work

The supplied data has previews rather than complete email bodies or recipient addresses. The reader therefore displays the supplied preview as its message content and labels the recipient “You.” Refresh messages are invented locally and clearly stay within this offline app.

No requested iPhone feature remains unfinished. Filters and navigation reset on relaunch; message states and received messages persist as required. Earlier web visual audits reported hidden swipe-action and menu overlap findings; the requested iPhone audit is clean.

## What would make this faster

A mail-oriented Exact starter covering both swipe directions, refresh, and persisted seed data would reduce setup. Explicit documentation of native reload/search behavior and gesture start coordinates would avoid test-driver detours. A shared semantic icon vocabulary for archive, tray, and flag would remove the need for small cross-platform SVG fallbacks.
