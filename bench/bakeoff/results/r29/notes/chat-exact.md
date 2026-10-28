# Chat (Exact): notes

Built, tested and installed on simulator `486FD498-…` as `Chat` (`dev.bakeoff.chat.bfd3468`).

## What's there
- `app.ts` imports `data/messages.json` (bundled), and keeps what changes in `app:/data/chat.json`:
  the messages sent (with Sam's replies) and the ids deleted. History plus that file is the
  thread, so sent messages, replies and deletions survive a relaunch.
- Rows are computed in `app.ts`: a day heading where the local day changes ("Today",
  "Yesterday", a weekday within the week, else "Wed, Jul 1"), and a `first` flag that adds space
  between runs of one sender.
- The view is the `chat-thread` recipe: a virtualized list with `scroll-start="end"` and
  `scrollFollowEnd`, and a `resizes-content` main so the composer rides the keyboard.
  Blank or whitespace-only text disables Send. The composer clears as soon as you send, and
  the list scrolls to the bottom row.
- Sam's reply: each send queues a due time 2 s later. A task ticks every 200 ms while any are due
  and sends `samReply`. "Sam is typing…" shows in the always-present last row while replies are
  pending. If you aren't scrolled far up, the reply scrolls into view.
- A long press opens a UIMenu with Copy (`copyText`) and Delete (red, kept).
- The list's `scroll` event sets `farUp` when you're more than about 1600 px from the end, which
  shows a floating ↓ button that scrolls smoothly to the newest message.
- Tests (`app.test.contract`): history and headings, empty send disabled, send, clear, typing,
  the reply at 2 s (not at 1 s), relaunch, and long-press delete kept across relaunch.

## What was hard / guessed
- The virtualized list allows exactly one `for` with a single root element. So the
  typing indicator and the scroll-to anchor are a synthetic last `Row` (`kind="bottom"`), and
  rows branch inside one `row`.
- I couldn't add randomness to `app.ts` or Contract, so Sam's canned reply is picked
  deterministically from a hash of your last message.
- Day boundaries use the zone's current `utcOffset`, so DST changes inside the history could put
  a message near midnight under the neighbouring day.
- The agent's screenshot of the iOS long-press menu shows the items without their platter,
  which looks like a capture artefact. The menu works: Copy and Delete were pressed in drives
  and tests.

## Not finished / known findings
- `verify ios` and `verify` report "overlap" findings at the largest text size: the composer or
  the ↓ button over a message. verify switches text size on a running app, which leaves the list
  off its end. A cold launch at the largest size, checked by hand, opens at the newest message
  and stays clear of the keyboard. Light, dark and touch-scrolling checks are clean
  ("scrolling: like UIKit").
- No per-message timestamps, read receipts, or bubble tails.

## What would have made it faster
- A chat recipe that already has a day heading, a typing row and a jump-to-newest button.
  Finding the single-`for`/single-root rule of virtualized lists cost two compile rounds.
- A documented way to set Dynamic Type for an `agent ios` drive. I set it with
  `simctl ui content_size`.
