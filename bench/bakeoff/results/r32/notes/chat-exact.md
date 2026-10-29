# Chat (Exact): notes

## What it is
- `app.contract`: one screen, a header titled "Sam", a virtualized transcript list
  (`scroll-start="end"` and `scrollFollowEnd=true`), a long-press menu (Copy/Delete), a floating
  "newest" button, and the composer.
- `app.ts`: the 520 bundled messages are imported from `data/messages.json`. What the person changes
  (messages sent, Sam's replies, deleted ids) is kept in `app:/data/thread.json`, then merged with the
  history on load. The data module also computes `startsDay` using the person's UTC offset.
- Sam's replies: each send adds a due time (now + 2 s) to a queue held in state. A `task … when typing`
  checks the queue every 200 ms and sends `samReply`. While the queue isn't empty, a "Sam is typing…"
  pseudo-row sits at the end of the list. Each message you send gets its own reply, 2 s later. The reply
  is "👍" or a canned line, chosen by a hash of your text (Contract has no random).
- Tests (`app.test.contract`, 6): history order and day headings, empty send disabled, send, clear,
  typing, reply at 2 s, survives a relaunch, delete persists, copy.
- `verify ios`: tests pass; no light or dark findings; scrolling "like UIKit". One finding remains at
  the largest text size: the floating jump-to-newest button overlaps a bubble. That is deliberate: it
  floats over the content.

## What was hard / guessed
- A virtualized list allows exactly one `for` with one root element. So the typing indicator can't
  be a separate row: it's a sentinel `Message` (id "typing") added to the end of the rows.
- Contract has no `last()`. I used `first(xs.slice(-1))`.
- "Far up" comes from `scroll=` with `ScrollEvent` (scrollHeight - scrollTop - clientHeight > 1200
  px). On a virtualized list scrollHeight is probably an estimate, but it behaved well.
- `scrollIntoView("transcript", "typing", block="end")` after a send brings the thread down even
  when you send from far up. I'm not sure a key that is only just being added scrolls reliably;
  scrollFollowEnd covers the usual case.
- Date headings use `formatDay(..., "day-time")` ("Today at 12:00 PM", "Jul 1, 2026 at 10:20 AM").
  iMessage's exact format ("Wed, Jul 1 at 10:20 AM") isn't available.
- The verifier flagged an opaque composer background (`lower-list-beside-fixed`): iOS runs the
  list under an unfilled composer and blurs it. I followed that advice, so the composer is unfilled.
- `overflow-wrap="anywhere"` (the recipe's) was flagged for breaking inside words, so I used
  `break-word`.
- The jump button's chevron is an `app.tamber` icon token (SF `chevron.down` plus an SVG), so the
  web/Linux builds aren't blank.

## Not done / limits
- Deleting is immediate with no confirmation, matching Messages' quick delete for a single message.
- No "Delivered"/read receipts or grouping of bubble tails.

## What would have made it faster
- A chat recipe that already includes a typing indicator, day separators and a jump-to-bottom
  button (all three are common in chat apps).
- `prefer` naming text sizes (`text-size accessibility-xxxl`) instead of `root-font-size <px>`.
