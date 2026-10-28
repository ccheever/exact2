# Chat: notes

Built with Exact (`app.contract`, `app.ts`, `app.test.contract`). Installed on simulator
486FD498-0E3E-4B64-8E4B-152BD8265EE7 as `dev.bakeoff.chat.bcd3c9c` ("Chat").

## What it does
- Shows the 520 messages from `data/messages.json` (imported into `app.ts` and bundled) as bubbles: mine on the right in the accent colour, Sam's on the left in grey. Messages from the same sender sit closer together. A centred "Oct 22, 2026 at 7:43 AM" / "Today at …" heading appears wherever the day changes, using the device's time zone.
- Uses a virtualized list with `scroll-start="end"` and `scrollFollowEnd`, so it opens at the newest message and follows new ones. The keyboard dismisses interactively, and `interactive-widget="resizes-content"` keeps the composer above the keyboard with the newest messages still showing.
- Composer: a growing textarea with a filled Send button that is disabled for blank or whitespace-only text. After a send the composer clears, keeps focus, and the thread scrolls to the end (even if you were scrolled up).
- Sam's replies: every message you send queues a reply due 2 s later. A "…" bubble plus "Sam is typing…" shows while any reply is pending. The reply is "👍" or a short canned line, chosen deterministically because Exact refuses randomness in the data module.
- Long-pressing a bubble opens a native menu with Copy (writes the clipboard) and Delete (red). Deleting a message from the shipped history is remembered too.
- When you are more than about 1.5 screens from the bottom, a round ↓ button appears and smooth-scrolls back to the newest message.
- Persistence: messages you send, Sam's replies and deletions are saved to `app:/data/thread.json`. The history file itself is never rewritten. Writes are serialized so a send and a reply can't race.
- 7 tests pass on web and iOS: load, blank refused, send + reply timing, one reply per message, survives relaunch, delete persists, copy.

## What was hard / guessed
- Getting Sam's reply "two seconds after each message" right when several messages are sent quickly. A `task … after(2000)` with a `key` would restart and drop replies, so I keep a list of due times and poll it every 200 ms while the list is non-empty.
- The composer's 44-pt touch target: the first fix (`min-height`) made the field two lines tall on iOS. Vertical padding on the textarea worked.
- `verify` flags the floating ↓ button as "overlapping" a bubble at the largest text size. That is what a floating button does, so I left it, but capped it at 44×44 so it doesn't grow to 112×77 under Dynamic Type.
- `overflow-wrap="anywhere"` (from the recipe) broke ordinary words mid-word at the largest text size, so I switched to `break-word`.
- I guessed the large-text drive preference (`prefer root-font-size 53`) after `content-size` was rejected.

## Not finished / known gaps
- `verify ios` reports scrolling-feel numbers against UIKit (deceleration, momentum hand-off, frame drops during status-bar scroll-to-top). It attributes most of them to Exact itself. I made the rows lighter (one fewer node each), but some hitching remains when flinging through all 520 rows.
- A reply still pending when the app quits is dropped (Sam doesn't "remember" to answer after relaunch).
- No read receipts, timestamps per bubble, bubble tails or avatar: they weren't in the spec.

## What would have made it faster
- A chat recipe that already includes a typing indicator, a jump-to-latest button and a long-press menu. The `apps/messages` example has them but is too elaborate to copy from.
- Documenting the `prefer` name for Dynamic Type next to `verify`'s largest-text report.
