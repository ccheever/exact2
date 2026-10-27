# Chat (Exact): notes

Built from the `chat-thread` recipe. It's installed on simulator CEDFB9C9-… as `dev.bakeoff.chat.bb90e69` ("Chat").

## What's there
- `app.ts` imports `data/messages.json` as the shipped history. After the first write (a send, Sam's reply or a delete), the whole thread is kept in `app:/data/thread.json`, so sent and deleted messages survive a relaunch.
- Bubbles come from a virtualized `list` with `scroll-start="end"` and `scrollFollowEnd`. Bubbles from the same sender sit closer together. A centred heading (`formatDay … "day-time"`) appears wherever the local day changes.
- The composer is the recipe's textarea with a Send button. Send is disabled for blank text, and the data module also refuses blank text. The composer clears and keeps focus after sending. `interactive-widget="resizes-content"` keeps the composer above the keyboard, and follow-end keeps the newest messages in view.
- Sam's replies: each send adds a due time (now + 2 s) to a queue. A 200 ms task runs while the queue is non-empty and sends one reply per due message. A "Sam is typing…" bubble shows as the last list row while replies are owed. Each reply is picked from a canned list (with several 👍) by hashing the sent text, because the data module has no randomness.
- Long-press menu: Copy uses `copyText`, and Delete is destructive and saves the change.
- Jump to newest: a floating chevron button appears when you are more than 1.5 screens above the end (`scroll=` event). It hides at `reachend`. Sending while far up also jumps to the end.
- Tests (`app.test.contract`): history loads, blank text can't be sent, send → clear → typing → reply → survives reload, and long-press delete survives reload. `verify` and `verify ios` pass the tests and report no light or dark findings. Scrolling is "like UIKit".

## What was hard or guessed
- **A virtualized list allows exactly one `for`, with one root per item.** The typing indicator couldn't be an extra row, so I added a placeholder message (`id: "typing"`) to the rows and wrapped each item in a `column`.
- **`key=` on a row is a key-press handler, not an identity key.** The compiler error explained it, but I tripped on it first.
- **`scrollIntoView(list, key, block="end")` aligns to the list's bottom edge, which is under the translucent composer.** The newest message ended up hidden behind the composer. Workaround: `offset=100000`, which is clamped to the end of the scroll range. A documented "scroll to end" for lists would have saved time.
- **`padding` with a computed value is refused.** I had to split it into `padding-top` etc.
- **Symbols:** there's no chevron-down role, so I used `symbol:sf/chevron.down`. That leaves one warning that the icon is blank on the web, which is fine for an iPhone-only app. I dropped Linux with `update --without linux`.
- **Calling an action from another action (`toNewest()` inside `post`) compiled.** I couldn't find this documented, so I guessed it was allowed.

## Not finished, or known rough edges
- At the largest text size, `verify ios` reports "overlap" findings: rows that have scrolled under the translucent composer. On screen at AX5 the layout looks right (the newest message sits above the composer). I treated these as intended, since iOS content does scroll under bars. The web verify also showed the jump button once on a largest-text relaunch.
- Replies still owed when the app quits are dropped. Delete doesn't ask for confirmation.
- Timestamps come from the device clock, so in agent drives (pinned clock) a new message's heading reads "Today at 12:00 AM".

## What would have made it faster
- A list-level "scroll to end" command that respects the composer's inset.
- In the chat-thread recipe: a typing indicator and a jump-to-latest button, plus a note on how to add a non-`for` row to a virtualized list.
- A symbol-role list that includes the common chevrons (up and down).
