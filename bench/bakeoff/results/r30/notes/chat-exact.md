# Chat (Exact) — build notes

Built, `bun exact.mjs verify ios` passed its 5 tests, and the app is installed on simulator
EF198926-1958-489D-A7A4-9196E454DD3A as `Chat` (`dev.bakeoff.chat.b77d1bd`).

## How the spec maps onto the app

- **History**: `app.ts` imports `data/messages.json`, so it ships in the bundle. Messages appear in
  file order, oldest first (the file is already sorted by `at` and by id). What the person adds is
  kept in `app:/data/thread.json` as `{added, deleted}` on top of the shipped history. The history
  itself is never rewritten.
- **Bubbles and date headings**: a virtualized `list` with `scroll-start="end"` and
  `scrollFollowEnd`. `loadThread(utcOffset)` marks `dayStart` (the first message of a local day) and
  `runEnd` (a wider gap after a run from one sender). A heading reads `formatDay(…, "day-time")`,
  e.g. "Today at 12:00 PM" or "Oct 24, 2026 at 2:17 PM".
- **Composer**: the recipe's textarea with a filled Send button. Send is disabled while the draft is
  only whitespace. Sending clears the draft at once, then scrolls to the end once the write lands.
  `interactive-widget="resizes-content"` keeps the composer above the keyboard, and the list runs
  under the composer with iOS's scroll-edge blur.
- **Sam**: each send adds a due time (`performanceNow() + 2000`) to a queue. A 100 ms task runs
  while the queue is non-empty and posts one canned reply per due time. "👍" is one of every three
  or so, picked deterministically because the data module may not use randomness. While replies
  are due, a typing bubble (three pulsing dots and "Sam is typing…") is the transcript's last row.
  Sam's reply only scrolls the list if the reader is already near the end.
- **Long press**: `contextPopover` opens a UIMenu with Copy (`copyText`) and Delete (red). Delete
  persists: shipped messages are kept as deleted ids, added ones are removed.
- **Jump to newest**: the list's `scroll` event sets `away` when the view is more than 1.5 screens
  (or 600 pt) above the end. A floating 44 pt chevron button then smooth-scrolls to the last row.
  The chevron is an `app.tamber` icon token: an SF Symbol on iOS, an SVG on the web.
- **Persistence**: a test reloads and checks that sent messages, Sam's replies and deletions are
  still there.

## What was hard / what I had to look up or guess

- A virtualized list allows only one `for` as its child, so I couldn't add the typing row after the
  loop. Instead the typing bubble is a synthetic `Message` with id `"typing"` appended in a
  `derive`.
- `scrollFollowEnd` didn't keep the end in view when the typing row and Sam's reply were appended
  right after my own explicit `scrollIntoView`; the new rows stayed below the fold. I scroll to the
  last row's key explicitly in each mutation's `then` action.
- At first I sorted messages by `at`. Tests start at a virtual epoch of 2026-01-01, so a message
  "sent now" sorted above the whole history. Insertion order is more robust anyway, e.g. against a
  device clock that is wrong.
- Small syntax issues the compiler explained well:
  - a block `match` has no `=>`
  - `animation` needs a declared `keyframes`
  - a `symbol:sf/…` icon is blank on the web without an icon token
  - finding how to set the largest text size in a drive took some searching: it is
    `prefer root-font-size 53`
- I guessed the canned replies and the jump threshold. Delete has no confirmation, as in the
  context-menu Delete of many chat apps.

## Known leftovers

- At the largest text size, `verify ios` still reports overlap and overflow findings. I checked the
  screens by hand and left them:
  - The composer and the floating jump button overlap rows that scroll underneath them, which is
    the intended Messages-style blur under the composer. The newest message stays fully visible
    above them.
  - The send button grows to 77 pt.
  - The jump button's chevron is now capped at 20 pt, a fix not re-run through `verify ios`.
- In agent drives the clock is virtual, so a new message's heading says "Today at 12:00 AM" there.
  On a real launch it uses the real time.
- I didn't add a test for Copy, since the clipboard isn't observable from a contract test.

## What would have made this faster

- A chat pattern (`pattern chat`) with typing indicator, jump-to-latest, context menu and
  persistence. The `chat-thread` recipe covers the basics only, and `apps/messages` is too elaborate
  to copy from.
- Having the "one `for` per virtualized list" rule and the `prefer root-font-size` op in the
  chat-thread recipe and the driving docs.
