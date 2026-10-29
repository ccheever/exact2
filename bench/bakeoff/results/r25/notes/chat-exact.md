# Chat (Exact): notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` tests).
`bun exact.mjs verify` and `verify ios` are both clean: 4 tests pass, there are no light or dark findings,
nothing spills at the largest text size, and scrolling is "like UIKit". The app is installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.chat.ba648e5` ("Chat").

## How each feature is done
- **History.** `data/messages.json` is imported into `app.ts`, so it is bundled with the app. Messages the
  person sends, Sam's replies and deleted ids go in `app:/data/chat.json`, which is merged with the history on
  every load. The data module works out `newDay`, using the device time zone through `Intl`, and `runEnd`,
  which tightens the spacing within a run of bubbles from one sender. A date heading uses
  `formatDay(..., "day-time")`, for example "Jul 3, 2026 at 11:20 PM" or "Today at 9:41 AM".
- **Transcript.** A `list virtualized=true scroll-start="end" scrollFollowEnd=true`, which opens at the
  newest message and follows new ones. It scrolls smoothly through all 520 messages to the first; the
  verify ios touch test agrees.
- **Composer.** A growing `textarea` with a filled Send button. Send is disabled when the text is only
  whitespace. `interactive-widget="resizes-content"` keeps the composer above the keyboard, and the
  follow-end list keeps the newest messages visible. Dragging the transcript dismisses the keyboard
  interactively.
- **Sam's replies.** Each send queues a due time 2 s later. A 100 ms task runs only while replies are
  pending: it writes Sam's reply when one is due and shows "Sam is typing…", both as a row above the
  composer and as a "typing…" subtitle in the navigation bar. Each message gets its own reply. Replies are
  "👍" (weighted) or a short canned line, chosen by a counter because Contract has no random.
- **Long press.** Pressing a bubble opens `contextPopover`, which is UIKit's context menu on iOS, with
  Copy (`copyText`) and Delete. Deletions are saved, and work on history messages too.
- **Jump to newest.** The list's `scroll` event sets `farUp` when the reader is more than 1200 px from
  the end. A round chevron button then appears and smooth-scrolls to the last key. If the reader sends a
  message while far up, the thread also jumps down.

## Hard parts / guesses
- **A virtualized list takes exactly one `for`.** The typing indicator could not go inside the
  transcript, so it is a row between the list and the composer.
- **Calling `scrollIntoView` while sending broke following the end.** The new message ended up below
  the fold, so I removed the call except when the reader is far up.
- **Long-press menu in the agent.** Under the agent, the long-press menu is drawn inline, not as a
  UIMenu. I had to read the host source (`ContextMenusIOS.swift`: "Under the agent this host is off")
  to learn this is by design. I could not see the real UIKit menu in a screenshot, so that part is
  trusted, not seen.
- **Fixes from verify.** `overflow-wrap="anywhere"` (from the chat recipe) was flagged for breaking inside
  words. `white-space="pre-wrap"` made text 6 px wider than its bubble at the largest size; `pre-line`
  fixed it. The composer field needed a 44 pt minimum height. `rem` is refused inside `min()`.
- **The chevron icon.** It needed an `app.tamber` icon token with an SVG, because `sf/` symbols are
  blank on web and Linux.
- **Guesses.** The canned replies, the 1200 px "far up" threshold, and showing a date heading with a
  time (iMessage style). I did not add a confirmation before Delete; it deletes straight away, as the
  spec lists it.

## Not done / limits
- Sam's pending replies are not saved: if the app quits within 2 s of a send, that reply is lost.
- Day headings use the current UTC offset for the label, so a heading next to a DST change can be an
  hour off in its time, though not in its day.

## What would have made it faster
- A chat pattern (`pattern chat`) with a typing indicator, a jump-to-bottom button and a long-press menu
  already wired in.
- Saying in the `menu` vocab that the agent draws context menus inline, so a screenshot is not a bug.
- A note in the `chat-thread` recipe that a virtualized list allows one `for`, and that `scrollIntoView`
  while sending works against `scrollFollowEnd`.
