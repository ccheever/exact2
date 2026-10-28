# Chat — notes

SwiftUI, iOS 26. Files: `Sources/App.swift` (entry), `Sources/Model.swift` (`ChatStore`: loading, persistence, sending, Sam's replies, rows with day headings), `Sources/ChatView.swift` (thread, bubbles, typing indicator, composer). `Resources/messages.json` is a copy of `data/messages.json` and ships as the seed history.

## How the spec maps to the code
- Thread: a `ScrollView` + `LazyVStack` with `defaultScrollAnchor(.bottom)` and `ScrollPosition(edge: .bottom)`, so it opens on the newest message. A day heading ("Today", "Yesterday", the weekday within the last week, then "Wed, Jul 1") appears wherever the day changes. Back-to-back messages from the same sender are grouped, with tighter spacing and a tail only on the last bubble.
- Composer: sits in `safeAreaInset(.bottom)`, which keeps it above the keyboard. Text is trimmed, and the Send button is disabled when the text is blank or only whitespace. When you send, the composer clears and the thread scrolls to the bottom. When the keyboard opens or a new message arrives and you are near the bottom, it scrolls to the bottom again.
- Sam: every message you send starts its own 2-second task, so sending several messages gets several replies. While any reply is pending, an animated "Sam is typing…" row shows. Replies are weighted toward 👍.
- Long-press: the context menu has Copy (pasteboard) and Delete (destructive, animated).
- The jump-to-newest button (a glass circle with a down arrow) appears when you are more than about 1200 pt above the bottom, measured with `onScrollGeometryChange`.
- Persistence: after every change the whole thread is written to Application Support/thread.json, and it is loaded in place of the seed on the next launch. This means deleted messages stay deleted too.

## Testing
No tap tool is available, so I added test hooks driven by launch arguments to `ChatView`: `-testScrollTop 1`, `-testJump 1`, `-testFocus 1`, `-testSend "text"`. I checked on the simulator: opening at the bottom, sending with the keyboard up (composer above the keyboard, newest message visible), the typing indicator and then the 👍 reply, survival across relaunch, scrolling to the first message, the jump button appearing and working, dark mode, and the AX5 text size. I could not test the long-press menu by touch, so I have only reviewed its code.

## Hard / guessed
- Guesses: the heading format (day plus the time of the first message, as in iMessage), the canned replies, the "far up" threshold, and that deletions should also persist.
- Sam's reply is not saved if you kill the app within the 2-second window.
- What would have made this faster: a way to tap and long-press on the simulator (for example idb), so I would not need launch-argument hooks.
