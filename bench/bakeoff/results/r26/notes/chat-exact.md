# Chat (Exact) — notes

The app is built and installed on simulator 498D6D1F-… as `dev.bakeoff.chat.b2e4fb3` ("Chat").
`app.contract` is the view, `app.ts` holds the data, and `app.test.contract` has 7 tests.
The tests pass on web and iOS. `verify` (web) is clean. `verify ios` has light and dark clean,
scrolling rated "like UIKit", and one leftover finding at the largest text size (see below).

## How it works
- **History:** `app.ts` imports `data/messages.json` and keeps a small overlay file,
  `app:/data/chat.json`, with the messages sent, Sam's replies, the deleted ids and an id
  counter. Sent messages, replies and deletes therefore survive a relaunch.
- **Transcript:** a virtualized `list` with `scroll-start="end"` and `scrollFollowEnd`.
  - A day heading appears where the local day changes, computed from `prevAt`. It uses
    `formatDay(... "day-time")`, e.g. "Today at 10:00 AM" or "Oct 25, 2026 at 1:00 PM".
  - Messages that are only emoji are drawn large without a bubble.
- **Sam's replies:** a `task … when typing key=replies` with `after(2000)`. Each message gets
  its own reply, and "Sam is typing…" shows while any reply is still owed. The reply is chosen
  deterministically (questions get a question-style answer), because `app.ts` refuses
  randomness.
- **Long-press:** opens a UIMenu with Copy (`copyText`) and Delete. Deleting works for both
  history and new messages.
- **Jump button:** a floating ↓ button appears when you are more than about 1.5 screens
  (at least 1000 pt) from the end.

## What was hard / guessed
- **`scrollIntoView(list, key, block="end")` ignores the composer's bottom inset.** On iOS it
  left the newest messages under the composer. The workaround is `offset=100`, which
  overshoots, and UIKit clamps the scroll to the real end.
- **No non-`for` rows in a virtualized list.** So the typing indicator sits between the list
  and the composer rather than inside the transcript.
- **The chat recipe's `overflow-wrap="anywhere"`** is flagged by verify ("Octo/ber"). I used
  `break-word` instead.
- **Glyph sizes at the largest text size:** I capped the send and jump icons
  (`font-size="min(-exact-body, …)"`).
- **Remaining finding:** `verify ios` still reports one overlap at the largest text size, on
  relaunch only. The jump button appears because the text size changes while the list is
  scrolled; the button has no testId on that node, so it can't be accepted. A real launch at
  that size opens at the end without the button (checked by hand).
- **Flaky findings:** the composer/send "overlap" findings at the largest size come and go
  between runs; they are content scrolling under the bar. They are accepted in `app.json`.
- **Day-heading test:** the heading for 25 Oct (message 448) can't be asserted in a test,
  because virtualized rows far from the end aren't built.

## What would have made it faster
- `scrollIntoView` honoring the list's content insets.
- A recipe for chat extras: a typing indicator, a jump-to-newest button and a message menu.
- A documented way to drive the largest text size in `agent ios`. I used
  `simctl ui content_size` and reset it afterwards.
