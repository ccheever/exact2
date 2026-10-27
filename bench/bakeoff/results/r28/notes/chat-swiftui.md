# Chat — notes

SwiftUI, iOS 26. `Sources/`: `Model.swift` (Message and Row types, JSON date coding), `ChatStore.swift`
(history, persistence, Sam's replies, day headings), `ChatView.swift` (thread, bubbles, typing indicator,
composer), `App.swift`. The 520 bundled messages are in `Resources/messages.json`, copied from `data/`.

## How it works
- The thread is a `ScrollView` + `LazyVStack` that opens at the newest message (`defaultScrollAnchor(.bottom)`,
  plus `.bottom` for `.sizeChanges`, so the bottom stays put when the keyboard shrinks the view). Rows are
  computed once each time the messages change: a date heading at each local-day change ("Today 1:28 PM",
  "Sunday 8:00 PM", "Wed, Jul 1 at 3:20 AM"), and consecutive messages from the same sender are grouped
  with tighter spacing and smaller corners.
- The composer is a `safeAreaBar` (iOS 26), so it rides above the keyboard and the content behind it is
  blurred. It is a multi-line `TextField` (Return adds a new line, as in Messages); Send is disabled for
  empty or whitespace-only text, and text is trimmed before sending.
- After you send, Sam answers 2 s later ("👍" or a short canned reply). Each message you send gets its own
  reply. While any reply is pending, a typing bubble plus "Sam is typing…" shows in the thread, and
  "typing…" shows under the title.
- Long-press a message for Copy or Delete. The preview takes the bubble's shape.
- A jump-to-newest button appears once you are about 1200 pt above the bottom.
- The whole thread, including sends and deletes, is saved to Application Support/messages.json after each
  change. That file replaces the bundled history from then on.
- Dark mode uses semantic colours. Dynamic Type: bubbles reflow at accessibility sizes, and the send
  button and large emoji scale with a cap.

## Hard parts / guesses
- `ScrollPosition(edge: .bottom)` and `scrollTo(edge:)` left the content shifted sideways by the
  horizontal padding and stopping short of the bottom (seen on the iOS 26 simulator). Fixed by scrolling
  to row ids (a 1-pt anchor view at the bottom) and using `contentMargins` instead of padding.
- `containerRelativeFrame` for the maximum bubble width also caused a shift. A `Spacer(minLength: 56)`
  on the other side does the same job reliably.
- `safeAreaInset` had no scroll-edge blur. `safeAreaBar` plus `scrollEdgeEffectStyle(.soft, for: .bottom)`
  fixed it.
- Guesses: the heading format (Messages-style date plus time), the canned replies, the "far up" threshold,
  and rendering emoji-only messages large without a bubble.
- The last dozen bundled messages all share one timestamp; they are kept in file order (a stable sort on
  date, then id).

## Testing / not verified
- `simctl` cannot tap or long-press, so I added launch-argument hooks (`-uiTestSend <text>`,
  `-uiTestFocus YES`, `-uiTestScrollTop YES`, `-uiTestReset YES`) and checked with screenshots: opens at
  the bottom; send, then typing indicator, then reply; persistence across relaunch; scroll to the first
  message, where the jump button appears; keyboard; dark mode plus AX5 text. `dev.sh` rebuilds, relaunches
  and screenshots.
- Not exercised on the simulator: the long-press menu (Copy/Delete), actually tapping the jump button, and
  interactive scrolling feel. They use standard SwiftUI APIs (`contextMenu`, `ScrollPosition.scrollTo(id:)`).

## What would have made this faster
- A way to tap or long-press on the simulator (idb or an XCUITest runner) to test gestures directly.
- Notes on the iOS 26 `ScrollPosition` edge quirks.
