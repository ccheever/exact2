# Chat — build notes

SwiftUI, iOS 26. `Sources/Store.swift` (model, Sam's replies, JSON persistence), `Sources/ChatView.swift` (UI),
`Resources/messages.json` (the shipped 520-message history).

## How it works
- The thread is a `ScrollView` + `LazyVStack` of precomputed rows (day headings + bubbles, with sender grouping),
  using `ScrollPosition`, `defaultScrollAnchor(.bottom)` for the initial offset and size changes (so it opens at the newest
  message and stays pinned when the keyboard shrinks the view), and `onScrollGeometryChange` to tell how far from the bottom
  the user is. The jump-to-newest button appears more than ~600pt from the bottom.
- Composer is a `safeAreaBar` (iOS 26 scroll-edge blur) with a multi-line `TextField`; send is disabled for blank/whitespace
  text, which is trimmed. Keyboard dismisses interactively by scrolling.
- Each send schedules a reply 2 s later ("👍" weighted, or a short canned line); a typing bubble with "Sam is typing…" shows
  in the thread (and "typing…" under the title) while any reply is pending.
- Long-press = context menu with Copy and Delete (plus matching VoiceOver actions).
- The whole thread (including deletions and Sam's replies) is saved to Application Support/thread.json on every change;
  the bundled JSON is only used on first launch.
- Dynamic Type: bubble radius and composer controls use `@ScaledMetric` (capped); checked at AX5 in dark mode.

## What was hard / guessed
- There's no touch automation available (no idb / XCUITest), so I added launch-argument test hooks
  (`-uiTestSend <text>`, `-uiTestFocus YES`, `-uiTestScrollTop YES`, `-uiTestDelete <id>`) and verified with
  `simctl launch` + screenshots: send/clear/typing/reply, persistence across relaunch, keyboard with newest messages visible,
  scroll to the first message + jump button, delete, dark mode, largest text size. The long-press menu itself and the
  smoothness of finger scrolling could not be exercised by touch; they are stock SwiftUI `contextMenu`/lazy scrolling.
- Guessed: day heading format (Today / Yesterday / weekday within a week / "Wed, Jul 1", with year if not this year), the
  canned replies, and that deletions should persist too. A reply still pending when the app is killed is dropped.
- Messages of 1–3 emoji render large without a bubble, like Messages.

## What would have made this faster
- A tap/long-press driver for the simulator (idb or an XCUITest runner) to test gestures directly.
