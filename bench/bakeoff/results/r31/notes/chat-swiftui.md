# Chat — notes

SwiftUI only (`Sources/App.swift`, `Model.swift`, `ThreadView.swift`). The 520 messages ship in `Resources/messages.json`.
After the first change, the whole thread (sent messages, Sam's replies, deletions) is saved to
Application Support/`thread.json` and loaded from there on later launches.

## How it is built
- Thread: `ScrollView` + `LazyVStack` with `.defaultScrollAnchor(.bottom)`, so it opens at the newest message.
  `.defaultScrollAnchor(.bottom, for: .sizeChanges)` keeps the bottom pinned when the keyboard shrinks the view.
  An iOS 18+ `ScrollPosition` drives scroll-to-bottom.
- The rows are a flat list of day headings ("Today", "Yesterday", the weekday name within the last week, otherwise
  "Wed, Jul 1", with the year added for other years) and bubbles. Messages from the same sender less than 10 minutes
  apart are grouped, with tighter spacing and smaller corners.
- Composer: `.safeAreaBar(edge: .bottom)` (iOS 26), so it sits above the keyboard and gets the scroll edge effect.
  The text field is multi-line. Send is disabled for whitespace-only text, and the text is trimmed before sending.
  Dragging the thread dismisses the keyboard interactively.
- Sam replies 2 seconds after each sent message: "👍" (weighted) or a short canned reply. "Sam is typing…" with animated
  dots shows while any reply is pending. The thread follows new messages if you sent them or are near the bottom.
- Long-pressing a bubble opens a context menu (Copy, Delete). The menu preview is clipped to the bubble shape, and the
  same actions are available to VoiceOver as custom actions.
- A glass "jump to newest" button appears once you are more than 1.5 screen heights above the bottom
  (found with `onScrollGeometryChange`).
- Works in dark mode and at the largest accessibility text size; the side gap and send button scale with the text size.

## Testing / what was hard
- There is no UI automation tool, so I couldn't tap. I added simulator-only launch arguments (`-testSend <text>`, `-testFocus`,
  `-testTop`, `-testScrollTo <id>`, `-testJump`, inside `#if targetEnvironment(simulator)`) to drive sending, the
  keyboard, deep scrolling and the jump button, and checked each with `simctl io screenshot`. With them I checked:
  sending plus the typing indicator and reply, staying sent after a relaunch, the keyboard keeping the newest messages
  visible, the top of history, the jump button, dark mode and AX5 text size.
- Never tested by hand: the long-press context menu (it's a standard `.contextMenu`) and how smooth real finger
  scrolling feels.
- Guessed: the wording of the day headings, the canned replies, and that a reply still pending when the app quits is
  simply dropped.

## What would have made it faster
- A way to send taps and gestures to the simulator (idb or similar), or permission to use XCUITest.
