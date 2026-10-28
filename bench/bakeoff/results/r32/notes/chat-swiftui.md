# Chat — build notes

SwiftUI app (`Sources/App.swift`, `Model.swift`, `ThreadView.swift`); `data/messages.json` ships as
`Resources/messages.json`. Once anything changes (a send, a delete, a reply from Sam), the whole thread is
saved to Application Support/`thread.json` on a background serial queue. On launch the app loads that file
and falls back to the bundled history if it isn't there.

## What was hard
- **Horizontal drift.** Opening with `ScrollPosition(edge: .bottom)` left the content scrolled 12pt
  sideways (`contentOffset.x == 12`), so Sam's bubbles touched the left edge and the right margin doubled.
  The fix was a position keyed on IDs (`scrollTo(id:anchor: .bottom)`) plus `.defaultScrollAnchor(.bottom)`.
  I found the cause by logging `onScrollGeometryChange`.
- **The "far up" distance.** `ScrollGeometry.contentOffset` is the raw UIScrollView offset, and
  `containerSize` excludes the safe-area insets. So the distance from the bottom is
  `contentHeight - offsetY - containerHeight - topInset`. I worked this out from logged values.
- **Composer backdrop.** With `safeAreaInset` the messages showed through the composer with no edge effect.
  Switching to iOS 26's `safeAreaBar` with `.scrollEdgeEffectStyle(.soft, for: .bottom)` fixed it.
- **No touch input on the simulator.** `simctl` can't tap, type or long-press. I added launch-argument
  hooks (`-demoFocus YES`, `-demoSend text`, `-demoDraft text`, `-demoScrollTop YES`, `-demoScrollIndex N`)
  so I could drive the app and screenshot it. They do nothing unless the argument is passed.

## Verified on the simulator (screenshots)
- Opens at the newest message.
- Day headings: "Today", "Yesterday", the weekday within the last week, and "Wed, Jul 1" style otherwise.
- Sending a message: the bubble appears, the composer clears, and the "Sam is typing…" indicator shows.
  About 2 seconds later Sam's reply arrives and the thread stays at the bottom.
- The keyboard opens with the composer above it and the newest messages still visible.
- Scrolling to the first message works, and the jump-to-newest button appears when far up.
- Sent messages are still there after a relaunch.
- Dark mode and the largest accessibility text size both look right.

## Not verified / guesses
- **Long-press menu.** Copy and Delete use the standard `.contextMenu`, but I couldn't press-and-hold in
  the simulator, so I never saw the menu. Both actions are also available as VoiceOver actions.
- **Scrolling smoothness** was judged from the jump to the top, not from a real finger flick.
- **Sam's replies:** one reply per message sent, chosen at random from a canned list. "👍" is weighted
  higher, and questions sometimes get yes/maybe answers. A reply that is still pending when the app is
  killed is lost.
- **Bubble grouping:** consecutive messages from the same sender less than 10 minutes apart get tighter
  spacing and smaller corners. This was my own call.
- **Send key:** the composer grows to 6 lines and Return inserts a newline, as in Messages. Only the
  send button sends.

## What would have made this faster
- A way to tap, type or long-press on the simulator (something like `simctl io tap`), or XCUITest.
- Documentation of `ScrollGeometry` field semantics, and of the `ScrollPosition(edge:)` horizontal quirk.
