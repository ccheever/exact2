# Inbox build notes

Completed and installed as **Inbox**, bundle ID `dev.bakeoff.inbox.b78dcb2`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used.

## Implementation

- All 320 original messages from `data/mail.json` are bundled through the data module, sorted newest first.
- Native inbox and Archive navigation, two-line preview space, unread markers/count, read/unread swipes, archive/restore, flags, unread/flagged filters, search, and message reading.
- Opening a message marks it read. Refresh creates two unique local unread messages newer than the existing mail, including on repeated refreshes.
- Read, flagged, archived, and received-message states use queued mutations and an atomically written local JSON file. Filter/search preferences also survive relaunch.
- Native colors and text styles support light mode, dark mode, and Dynamic Type. Preview text can grow beyond two lines at accessibility sizes so it remains readable.

## Decisions and assumptions

The supplied data has no separate message body or recipient address. The reader displays the complete supplied preview as the message text and identifies the recipient as “You.” The unread count counts messages currently in Inbox; archived unread messages retain their state and count again if restored. Refreshing receives mail into Inbox.

## What was hard / what I looked up

- Read Exact’s start guide, started from its list-detail pattern, and looked up swipe actions, persistence, navigation, date formatting, menus, icon tokens, and simulator driving.
- The first toolkit commands needed access to Exact’s shared build cache outside the app workspace. Native compilation was initially several minutes; later rebuilds took seconds.
- Changing assets during a background build invalidated its receipt. Finishing those edits and rebuilding resolved it.
- Replaced a segmented filter that clipped at very large type with a native filter menu. Native list rows reserve two preview lines and expand with text.
- Portable tests initially assumed native relaunch preserved the reader route and that hidden navigation screens vanished from the test tree. Adjusted them to reopen messages and assert the active folder’s data.
- Real gesture tests must begin far enough toward the appropriate edge to complete a full swipe without leaving the simulator viewport. Added stable identifiers to those row regions for testing.

## Validation

- `bun exact.mjs test web`: **9 passed, 0 failed**.
- `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios`: **clean**, **9 passed, 0 failed**; no light/dark layout findings; nothing spills at accessibility-extra-extra-extra-large (53 pt body).
- Real simulator touches confirmed pull-to-refresh adds exactly two messages, full left swipe archives, and full right swipe updates unread state/count.
- Relaunch tests confirmed saved read, flagged, archived, and refreshed messages.
- Native scrolling verification found nothing for the app to fix. The busy shared machine prevented timing judgment; Exact reported its own rubber-band/status-bar-scroll differences from UIKit.
- Final `bun exact.mjs ios` succeeded and installed the verified app on the assigned simulator.

No requested iPhone feature remains unfinished. Web verification’s wide split-view checks reported covered, offscreen swipe controls; the iPhone verification is clean.

## What would have made this faster

A mail-specific Exact starter with bidirectional swipes and refresh, a project-local build cache, clearer stale-build recovery, and explicit documentation of native test-tree/relaunch behavior would have reduced iteration time. The targeted vocabulary commands and native incremental plan updates were useful.
