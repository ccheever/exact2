# Chat (Exact): notes

Built on Exact: `app.contract` is the view, `app.ts` the data module and `app.test.contract` holds 8 tests.
`bun exact.mjs verify` (web) and `verify ios` are both clean. On iOS that means 8/8 tests, light and dark,
nothing spills at the largest text size, and "scrolling: nothing for the app to fix". The app is
installed on simulator 498D6D1F-… as `dev.bakeoff.chat.bb57259` ("Chat").

## How each feature is done
1. **Thread.** `data/messages.json` is imported by `app.ts`, so it ships in the bundle. Rows are a
   virtualized `list` with `scroll-start="end"` and `scrollFollowEnd`. Day headings are worked out in
   `app.ts` from the device's UTC offset and show as "Oct 24, 2026 at 2:17 PM" or "Today at …" using
   `formatDay`. A message that is only one to three emoji shows large with no bubble; bubbles in a run
   from one sender get tighter corners.
2. **Composer.** Send is disabled when the text is empty or only spaces, and the data module refuses
   it too. Sending clears the field straight away and scrolls to the new message.
3. **Keyboard.** `interactive-widget="resizes-content"` keeps the composer above the keyboard. The
   list stays at its end, and swiping down on it dismisses the keyboard.
4. **Sam's reply.** Each send queues a reply due 2 s later. A task ticks every 100 ms only while a
   reply is owed. The canned reply is picked by a counter, because the data module can't use
   randomness. The "Sam is typing…" bubble with animated dots sits just above the composer. Owed
   replies are saved, so a reply still arrives after a quit.
5. **Long-press.** Each message opens UIKit's context menu with Copy and Delete (red). Deleting a
   shipped message records its id; deleting a sent one removes it.
6. **Scrolling.** The list is virtualized. A round ↓ button appears once you are more than about 900 pt
   from the end and scrolls smoothly back to the newest message.
7. **Saving.** `app:/data/thread.json` keeps what you sent, Sam's replies and what you deleted. Writes
   go through a promise chain, so a reply arriving during a send can't overwrite it.

## What was hard / guessed
- **Long-press menu.** I first used one shared menu plus a `contextmenu=` handler that remembered
  which message was pressed. With a real long press on the simulator, the handler's state change made
  the menu draw inline as a plain web popover instead of UIKit's menu. Giving each row its own menu,
  with the message passed straight to the buttons and no handler, fixed it. This cost the most time,
  and I'm not certain of the cause. I also couldn't capture UIKit's menu in a screenshot: the agent's
  window capture doesn't include it, and a drive quits the app when it ends, so `simctl` grabbed the
  home screen. I checked Copy and Delete by driving them (the logs show `copyText`, and the row is
  removed).
- **Virtualized lists** allow exactly one `for`, so the typing indicator sits outside the list, above
  the composer, rather than as the last row.
- **"Two seconds after each message."** I read this as one reply per sent message, each 2 s after it,
  not one reply per burst.
- The first `--prebuild` failed with `AppMismatch` because `appId` in `app.ts` still had the template
  id. The manifest and the module have to change together before the first build.
- At the largest text size, `overflow-wrap="anywhere"` (as in the chat recipe) broke words mid-word,
  and an image with no set size grew to 64 pt. Verify caught both.

## Not done / limits
- No bubble tails and no per-message timestamps (swipe-to-reveal times, as Messages does).
- Copying worked in the drive (the logs show `copyText`), but I didn't paste anywhere to confirm the
  clipboard contents.
- Day headings use the current UTC offset, so a day boundary can be an hour off across a DST change.

## What would have made this faster
- A chat recipe that already covers a long-press menu inside a virtualized list, a typing indicator and
  a jump-to-latest button. Each was a separate lookup.
- A warning, or an agent screenshot, that shows the real UIKit context menu, so I wouldn't have had
  to work out from inline drawings whether the menu was native.
