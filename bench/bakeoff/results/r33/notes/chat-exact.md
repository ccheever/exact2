# Chat (Exact) — build notes

## What was built
- `app.contract`: one screen. A virtualized `list` (`scroll-start="end"`, `scrollFollowEnd`) of bubbles, mine
  on the right in the accent colour and Sam's on the left in the system fill. A centred date heading
  (`formatDay … "day-time"`) appears wherever the calendar day changes. Consecutive bubbles from the same
  sender sit closer together, and a message that is only emoji is drawn large with no bubble, as Messages does.
- The composer is the `chat-thread` recipe's: `interactive-widget="resizes-content"` keeps it above the keyboard,
  Send keeps focus, and dragging the thread dismisses the keyboard interactively. Send is disabled for empty or
  whitespace-only text, and the data module also refuses it.
- Sam: each send queues a due time (`replyDue`). A task armed while the queue is non-empty runs every 200 ms and
  sends Sam's reply once each message's due time arrives, so every message gets its own reply two seconds later.
  A "Sam is typing…" bubble is the last row while any reply is pending. Replies are canned ("👍" and short
  phrases), picked deterministically because Contract and the data module both refuse randomness.
- A long press opens a UIMenu with Copy (`copyText`) and Delete.
- The jump-to-newest button floats over the thread once you are more than 1.5 screens from the end.
- Persistence: `app.ts` imports `data/messages.json` as the shipped history. `app:/data/thread.json` holds
  the sent messages, Sam's replies and the deleted ids. Every write goes through one `queue` mutation, so a send
  and a reply never race on the file. Sent messages follow the history in the order they were sent, not by
  timestamp, so a device clock set earlier than the history can't move them into the middle of the thread.
  History messages that share a timestamp keep their id order.
- Tests (`app.test.contract`, 6): load order and day headings, the empty-send rule, send → typing → reply,
  survival across a relaunch, delete (and that it is kept), copy. `verify` and `verify ios` both pass their tests.

## What was hard / guessed
- A virtualized list allows exactly one `for` with a single root element, so the typing indicator can't be a
  separate row after the loop. I add it to the data as a pseudo-message (`id == "typing"`) instead.
- Per-corner `border-*-radius` compiles, but on iOS the bubbles still drew uniformly rounded, so grouped bubbles
  have no tail or flattened corners. I separate the groups with spacing only.
- `verify ios` at the largest text size reports overlaps: the floating jump button over bubbles (it floats on
  purpose), and the composer over bubbles after the text size changes while the thread is open. When the app is
  launched at the largest size the layout is correct (checked by hand on the simulator).
- I couldn't screenshot the native long-press UIMenu: the agent paints its own stand-in. Delete and Copy were
  exercised through the agent on iOS and in the web tests.
- I guessed the threshold for "far up" (1.5 screens) and the list of canned replies.
- I guessed that deleting a message should persist and that Delete needs no confirmation (it comes from an
  explicit long-press menu).

## Not finished
- Nothing from the spec is missing. I didn't add bubble tails, read receipts or per-message timestamps.

## What would have made it faster
- A note in the `chat-thread` recipe that a virtualized list takes one `for`, and a pattern with a typing
  indicator or footer row.
- A way to drive the dynamic text size directly from `agent ios` (I used `xcrun simctl ui … content_size`).
- Saying whether per-corner radii are supported on iOS.
