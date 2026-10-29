# Chat — build notes

SwiftUI only, no UIKit views (`UIPasteboard` for Copy is the only UIKit API used). Files:
- `Sources/ChatStore.swift` — `@Observable` store: loads `messages.json` from the bundle (copied from `data/` into
  `Resources/`). The first time anything changes it writes the whole thread to Application Support (`thread.json`)
  and loads from there afterwards, so sent messages, Sam's replies and deletions all survive a relaunch. The store
  also schedules Sam's reply 2 s after each send: "👍" half the time, otherwise a canned line.
- `Sources/ChatView.swift` — `ScrollView` + `LazyVStack` with `defaultScrollAnchor(.bottom)` and a `ScrollPosition`.
  Day headings (Today / Yesterday / weekday / date, plus the time) appear where the day changes. Back-to-back
  bubbles from the same sender are grouped. Emoji-only messages show large with no bubble. Long-press opens a
  context menu with Copy and Delete; VoiceOver gets the same two actions. A "jump to newest" glass button appears
  when you are more than about 900 pt from the bottom. The composer sits in a `safeAreaBar`, so it stays above the
  keyboard and content blurs behind it. The keyboard closes interactively when you scroll. While Sam is "typing",
  there is an animated dots bubble with "Sam is typing…" and the title shows "typing…".
- `tools/icon.swift` draws the app icon PNGs in `Resources/`.

## Tested on the simulator
Opens at the newest message. Sending scrolls to the new message and clears the composer; a blank composer keeps
Send disabled, and the store also ignores whitespace-only text. The typing indicator shows and the reply arrives
after 2 s. The newest messages stay visible above the keyboard. Scrolling to the first message (Jul 1) and jumping
back both work. Messages persist across relaunch. Checked in dark mode and at AX5 text size.

`simctl` cannot tap or long-press, so the app reads launch arguments (`-testSend <text>`, `-testFocus YES`,
`-testScrollTop YES`, `-testJumpBack YES`) to drive the same code paths. Because of that, the context menu
(Copy/Delete) and the real button taps were checked by reading the code, not by actually pressing them.

## Hard / guessed
- With no UI automation, testing interaction meant adding these launch-argument hooks.
- Whether the bottom anchor follows the viewport when the keyboard opens: I also scroll to the bottom explicitly
  when the composer gets focus, if you were already near the bottom.
- New messages pull the view down only if you are near the bottom (or you sent the message). If you have scrolled
  up, Sam's reply does not yank you back down.
- Guesses: which canned replies to use, the "far up" threshold, and grouping messages less than an hour apart.
  Many of the last history messages share one timestamp; they keep file order (sorted by date, then id).
- If the app is killed during the 2-second wait, that pending reply is lost.

## Would have made this faster
A way to send taps and long-presses to the simulator (e.g. `idb`, or XCUITest available in the skeleton).
