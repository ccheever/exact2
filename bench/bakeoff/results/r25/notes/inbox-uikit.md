# Inbox build notes

Finished and installed as `dev.bakeoff.inbox.b4beb95` (Inbox) on simulator
`486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final production build starts with
the original 320 messages, 109 unread and 22 flagged. Test changes were cleared
before this final installation. No other simulator was booted, erased, or used.

## What was hard

- iOS 26 places the large navigation title in scroll content. An opaque scroll-edge
  navigation appearance covered it; using a transparent scroll-edge appearance
  fixed the title. Search also needed explicit stacked placement for this layout.
- Self-sizing rows need a different sender/date arrangement at accessibility sizes.
  Names, subjects, reader metadata, and empty states wrap, and the reader scrolls.
  Selected filter text uses a different foreground color in dark mode for contrast.
- Opening or changing a message can remove it from an active filter immediately.
  Actions capture stable message IDs before updating state to avoid stale indexes.
- State is saved atomically after every change, including generated refresh messages.

## What I looked up or decided

Read the supplied README and build script and consulted `simctl` help for appearance,
content size, screenshots, and simulator commands. No external packages or network
services are used.

The dataset provides previews but no separate body or recipient address. The reader
shows the complete supplied preview as the message text and uses “To: You.” Refresh
adds two locally written unread messages with unique IDs and dates newer than the
existing mailbox. Counts and filters apply to the current mailbox. Archive also
supports restoring a message to Inbox.

## Verification

An XCTest-free, compile-time-only harness exercised 20 action/model checks in the
running simulator and two further checks after terminating and relaunching the app.
It covered sorting, bundled data, unread counts, opening messages, both read-status
swipe handlers, flagging, filters, archiving, restore, search, empty results, refresh,
and persisted states. UIKit supplies the actual swipe gesture implementation.

Visually checked light/dark mode, the largest accessibility text size, reader,
empty Archive, and the native search keyboard. No runtime Auto Layout conflict logs
were found. See `Tests/RESULTS.txt` and `Tests/Screenshots/`. The verification harness
is excluded from the final production build.

## Unfinished work

Nothing outstanding within the requested scope.

## What would have made this faster

A documented simulator gesture driver would make end-to-end swipe and scrolling
checks easier. A seed schema with a separate message body would remove the need to
interpret preview text as the available message content. A starter UIKit scene
manifest and iOS 26 navigation appearance example would save setup time.

## Rebuild

```
SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run
```
