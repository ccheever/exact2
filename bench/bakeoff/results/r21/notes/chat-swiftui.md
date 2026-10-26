# Notes

## What's built
SwiftUI app (`Sources/App.swift`, `Model.swift`, `ChatView.swift`). The history ships as `Resources/messages.json`.
On first launch the app loads it. After that, the whole thread (sent messages, Sam's replies and deletions) is
saved to `Application Support/messages.json`, and that file is loaded on later launches.

- Bubbles: mine on the right in the accent colour, Sam's on the left in grey. Consecutive messages are grouped and
  the last one in a run gets a "tail" corner. There is a day heading at each date change (Today / Yesterday /
  weekday / full date). Tapping a bubble shows its time.
- The thread is a `ScrollView` + `LazyVStack` with `defaultScrollAnchor(.bottom)` for the initial offset and for
  size changes, so it opens at the newest message and stays pinned when the keyboard opens.
- The composer is a `safeAreaInset`: a multi-line field plus a send button. The button is disabled when the trimmed
  text is empty, and the store also rejects empty text. Swiping down on the thread dismisses the keyboard.
- Sam replies 2 s after each send, with "👍" weighted more heavily than the other canned replies. A typing bubble
  (animated dots plus "Sam is typing…") shows while replies are pending. Each send gets its own reply.
- Long-press opens a context menu with Copy and Delete. Both are also VoiceOver actions.
- A jump-to-newest button appears when you are more than a screen up. It shows a count of Sam replies that arrived
  while you were scrolled up. Replies don't yank you down if you are reading history; your own sends always scroll
  to the bottom.
- Tested on the simulator in light and dark mode, at AX-XXXL text, with the keyboard up, through a relaunch
  (persistence), and scrolling to the first message and back.

## What was hard / guessed
- `ScrollGeometry` semantics: `containerSize` excludes the content insets, and `contentOffset` is measured from
  the top inset. I had to log the values to get "distance from bottom" right; the obvious formula was off by the
  composer and keyboard insets.
- Initialising `ScrollPosition(edge: .bottom)` made the content render shifted left by the horizontal padding.
  `ScrollPosition(idType:)` fixed it.
- Jumping to the bottom from far up lands short, because lazy rows have estimated heights. `scrollToBottom`
  re-issues `scrollTo(edge: .bottom)` a few times until the measured distance reaches 0.
- There is no tap-injection tool, so the app has a launch-argument test hook
  (`-demo "focus;type:Hi;send;wait:2;up:200;jump;top"`) to drive focus, typing, sending and scrolling for
  screenshots. The long-press context menu is standard `.contextMenu` and was not exercised on the simulator.
- Guessed: deleting history messages also persists. Pending replies are dropped if the app is killed within 2 s.

## What would have made this faster
A way to inject taps and long-presses into the simulator (e.g. `idb` or `simctl io tap`), and documentation of
`ScrollGeometry` inset semantics.
