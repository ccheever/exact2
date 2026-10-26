# Chat (Exact): notes

Built with Exact. The app is `Chat` (`dev.bakeoff.chat.b3f42e4`) and is installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256. Final `bun exact.mjs verify ios` was clean: 6 tests passed,
light and dark had no findings, nothing spilled at the largest text size, and scrolling showed
"nothing for the app to fix".

## How each feature is done
- **Thread** (`app.contract`): a `list virtualized=true scroll-start="end" scrollFollowEnd=true`
  of bubbles. Mine are on the right in the accent colour; Sam's are on the left in
  `-exact-secondary-fill`. A date heading ("Today at 3:20 PM", "Sep 7, 2026 at 4:30 AM") sits
  above the first message of each local day. The data module works out `newDay` from the zone
  offset that `exactTime()` gives.
- **History** (`app.ts`): imports `data/messages.json` and ships it. The thread is written to
  `app:/data/thread.json` on the first send or delete; until then the shipped history is the
  thread. Because it's a file, sent messages, Sam's replies and deletions survive a relaunch.
- **Composer**: a growing `textarea` with a filled Send button. Send is disabled for empty or
  whitespace-only text. The composer clears and keeps focus after sending, and the thread
  scrolls to the new message. `interactive-widget="resizes-content"` keeps the composer above the
  keyboard, and `scrollFollowEnd` keeps the newest messages visible when it opens. Dragging the
  thread down dismisses the keyboard interactively.
- **Sam's reply**: every send adds a due time 2 s ahead to a queue. A `task` runs while the queue
  is non-empty and sends each reply when its time comes. So every message gets its own reply,
  even several sent quickly. The reply is "👍" or a canned line, picked by the seeded `roll`
  from the random-rolls recipe. While a reply is pending, "• • • Sam is typing…" shows above
  the composer and the navigation bar's subtitle reads "typing…".
- **Long press**: one `role="menu"` popover serves every bubble through `contextPopover`, with
  Copy (`copyText`) and a red Delete.
- **Jump to newest**: the list's `scroll` event sets `farUp` when you are more than 1500 px from
  the end. A round button at the bottom right then scrolls smoothly back to the last message.

## What was hard / what I had to look up or guess
- **Long-press menu under the agent:** in agent drives it shows as a painted inline popover,
  even with `--touch platform`. That looked like a bug until I read
  `host/apple/.../ContextMenusIOS.swift`: the host turns off `UIContextMenuInteraction` when
  `ExactEnv.agentMode` is set. The installed app uses UIKit's native context menu, but I could
  not screenshot it through the tools. The docs say iOS "presents UIKit's context menu"; they
  never mention that agent drives don't.
- **One-`for` limit:** a virtualized list allows only one direct `for`, so the typing indicator
  can't be a trailing bubble inside the transcript the way it is in Messages. It sits between
  the transcript and the composer instead.
- **Web-only findings:** web `verify` at the largest text size reports a few bubbles "9 px wider
  than the box" and the floating jump button overlapping a message (by design). The iOS
  `verify` and `check` at AX5 found nothing, and the screenshots look right. I left these
  web-only findings.
- **Guessed:** the canned replies, the 1500 px "far up" threshold, and the date-heading format
  (Exact's `formatDay … "day-time"`).

## Not finished / limitations
- If the app is quit during the 2 s wait, Sam's reply is lost. The pending queue is in memory
  only.
- Delete doesn't ask first, which matches iOS context-menu deletes for single messages.
  There's no undo.

## What would have made this faster
- A note in the menu docs that agent drives paint context menus instead of using UIKit, and
  a way to screenshot the native one.
- A chat pattern (`bun exact.mjs pattern chat`) with date separators, a typing indicator and
  a long-press menu built in. The `chat-thread` recipe was close but minimal, and the bundled
  `apps/messages` is a pixel-exact clone that is too large to borrow from.
