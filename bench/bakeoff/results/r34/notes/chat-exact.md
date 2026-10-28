# Chat (Exact): notes

Built as `dev.bakeoff.chat.bfbabef` / "Chat" and installed on simulator 498D6D1F-….

## What's there
- `app.contract`: the thread is a virtualized `list` (`scroll-start="end"`, `scrollFollowEnd`). Mine are
  accent bubbles on the right, Sam's are fill bubbles on the left. A date heading
  (`formatDay … "day-time"`, e.g. "Oct 25, 2026 at 3:25 AM" / "Today at 4:42 PM") sits above the first
  message of each local day. The composer is the chat-thread recipe's textarea and Send button, and it
  stays above the keyboard via `interactive-widget="resizes-content"`.
- Sam replies: each send bumps `owed` and re-arms a 2 s `after` task. A "Sam is typing…" row shows
  while `owed > 0`, then a canned reply is written for each message sent ("👍", "Sounds good!", …).
  The reply is picked deterministically, because the data module may not use randomness.
- Long-pressing a bubble opens a context menu (UIMenu on iOS) with Copy (`copyText`) and Delete.
- A floating ↓ button appears when you're more than ~1500 px from the end and scrolls smoothly back
  to the newest message.
- `app.ts`: imports `data/messages.json` as the history and keeps sent messages, Sam's replies and
  deleted ids in `app:/data/thread.json`. That data survives relaunch, and deleting a history
  message also survives relaunch.
- 7 tests in `app.test.contract`. They pass on web and iOS (`verify`, `verify ios`). Scrolling
  measures "nothing for the app to fix".

## What was hard or guessed
- **Clock vs. order:** the first version sorted all messages by timestamp. Agent drives pin the
  epoch to 2026-01-01, which is earlier than the history, so a sent message sorted to the top and
  "vanished". Now the history is sorted and sent messages are appended in send order. That is also
  right on a device whose clock is wrong.
- **Virtualized lists in tests:** rows outside the window aren't in the tree. I had to learn
  `tap "transcript" into "<key>"` and add `clock settle` before checking the scroll-driven
  `to-newest` button.
- **Small things the compiler taught me:** a virtualized list allows exactly one `for`, so the typing
  indicator is a row under the list, not a list row. A `textarea` has no `submit`, so Return inserts
  a newline and Send sends. There is no `xs[i]` (use `xs.at(-1)` plus `match`), and a few colour and
  symbol role names differ from UIKit's.
- **Guessed:** "far up" means 1500 px from the end. A heading shows date and time (iMessage style). If
  several messages are sent within 2 s, Sam replies to each of them once the timer fires.

## Left as is
- `verify` reports "overlap" for the composer and the ↓ button over messages. On iOS the transcript
  scrolls under the translucent composer, as in Messages, and the ↓ button floats by design.
- `contract verify` reports DIVERGE between Exact's two internal semantics engines on
  `scrollIntoView(list, key)` in a mutation's `then` action. The app behaves correctly on web and iOS.
  This looks like an Exact-internal disagreement, not an app bug.
- One compiler warning: `symbol:arrow.down` is aliased to the `download` role. I kept the plain arrow
  because it reads as "jump down", not "download".

## What would have made it faster
- A chat pattern (`pattern chat`) with a typing indicator, a long-press menu and a jump-to-latest button.
- A doc note that agent and test drives pin the epoch, next to the dates recipe.
