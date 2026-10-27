# Chat: build notes

Built with Exact from the `chat-thread` recipe. The app is `Chat` (`dev.bakeoff.chat.b51f9c1`),
installed on simulator C2188CC6-FBE9-409A-92A8-ABD89AC2C432. `verify` (web) and `verify ios`:
5/5 tests pass, no check findings in light or dark mode, nothing spills at the largest text size
on iOS, and scrolling is reported as "like UIKit".

## How each feature is done
1. **Bubbles and day headings.** Mine are on the right in the accent colour, Sam's on the left in
   grey, in a virtualized `list` (`scroll-start="end"`) so it opens at the newest message. `app.ts`
   gives each row the previous row's time and sender. The view puts a "Jul 3, 2026 at 11:20 PM" /
   "Today at …" heading wherever the local day changes, and leaves a slightly bigger gap where the
   sender changes. 60 of the shipped messages share one timestamp; they keep file order.
2. **Composer.** Leading and trailing spaces are trimmed. Send is disabled while the text is empty
   or blank. The field clears and keeps focus. `scrollFollowEnd` keeps the thread at the bottom. A
   message sent while scrolled far up scrolls down to it (`afterSend`).
3. **Keyboard.** `interactive-widget="resizes-content"` keeps the composer above the keyboard and
   the newest messages visible (checked on the simulator). Dragging the list down dismisses the
   keyboard interactively.
4. **Sam's replies.** Each send queues a reply due 2 s later. A task polls every 200 ms while one
   is owed and shows "••• Sam is typing…". The reply is "👍" or a short canned line. Neither
   `app.ts` nor Contract allows randomness, so a hash of your text picks it.
5. **Long press.** A long press opens a native context menu with Copy (`copyText`) and Delete.
   Deleting also works for shipped messages; their ids are kept in a "deleted" list.
6. **Long history.** A virtualized list keeps scrolling smooth. A glass ↓ button appears when the
   newest message is more than two screens away and scrolls smoothly back to it.
7. **Persistence.** Sent messages, Sam's replies and deletions live in `app:/data/thread.json`.
   The shipped history is bundled as `data/messages.json`, imported by `app.ts`. Writes are
   serialized in `app.ts`, so a send and a reply landing together can't drop each other.

## What was hard or guessed
- **The composer's background.** Exact warns (`lower-list-beside-fixed`) that a filled bar under
  the list breaks iOS 26's scroll-edge effect. I tried the unfilled field and a glass capsule. In
  both, bubble text showed through right behind the placeholder, which is hard to read, so I kept
  an opaque background bar. That compiler warning is the one problem `verify` still reports, and
  it's deliberate.
- **The ↓ button at the largest text size.** On the web only, after a relaunch, the floating
  button overlaps a bubble at the largest text size. That's what a floating button does; iOS
  reports nothing. I capped its glyph at 22 px.
- **Test syntax.** Block `match` has no `=>`, and there is no `last()` (I used
  `first(xs.slice(-1))`). A virtualized list allows only one `for`, so the typing indicator sits
  under the list rather than as its last row.
- **Message ids.** Ids are predictable (`m1`, `s2`, …) so tests can name them.
- **Not done:** a reply still owed when the app quits (within 2 s of sending) is dropped. Sam
  doesn't reply after a relaunch.

## What would have made it faster
- A `chat-thread` recipe covering a typing indicator, a jump-to-newest button and a composer
  background that meets the scroll-edge rule while staying readable.
- `agent ios` screenshots that show real glass rather than "approximate" pixels, so materials can
  be judged without guessing.
- The first `--prebuild` failed because I changed `app.json`'s id right after starting it. Having
  it say so up front, instead of later as an `AppMismatch`, would have saved a rebuild.
