# Chat — build notes

## What's here
- `Sources/ChatStore.swift`: the model. On first launch it loads the bundled `Resources/messages.json` (a copy of
  `data/messages.json`), sorted by `(at, id)`. After that, the whole thread (sent messages, Sam's replies,
  deletions) is saved to `Application Support/thread.json` on every change and loaded from there on later launches.
  It also precomputes the rows: where day headings go and how bubbles group.
- `Sources/ThreadView.swift`: `ScrollView` + `LazyVStack`, `defaultScrollAnchor(.bottom)`, `ScrollPosition` for
  programmatic scrolling, `onScrollGeometryChange` to work out the distance from the bottom (the jump button shows
  when you are more than about 1200 pt up), and the composer in `safeAreaInset(edge: .bottom)` so it rides on
  top of the keyboard. Keyboard dismisses interactively when you scroll.
- `Sources/MessageViews.swift`: bubbles (grouped corners, jumbo emoji), day headings (Today / Yesterday / weekday /
  date), typing indicator, composer (multi-line field with a Liquid Glass send button that is disabled for
  whitespace-only text).

## Behaviour decisions (guesses)
- Every send gets its own reply 2 s later. "Sam is typing…" stays up while any reply is pending. Replies are picked
  at random from a canned list weighted toward "👍".
- New messages scroll the thread to the bottom only if you were already near it. Your own sends always do.
- Deleting a bundled history message also persists. If you quit while a reply is pending, that reply is lost.
- Return in the composer inserts a newline, as in Messages. You send with the button.
- Message text trims leading and trailing whitespace before sending.
- The data has many messages with identical timestamps, so ordering falls back to `id`.

## What was hard
- The first version used `ScrollPosition(edge: .bottom)` as its starting value. Combined with
  `defaultScrollAnchor(.bottom)`, that left the initial content shifted sideways by the horizontal padding, with a gap
  under the last message. Starting with `ScrollPosition(idType:)` and letting `defaultScrollAnchor` handle the first
  position fixed it.
- `simctl launch` won't restart an app that is already running, so the first "after" screenshot was stale. The fix
  was to terminate the app before every launch (`tools/run.sh`).
- simctl has no way to tap or type, so I added launch-environment hooks (`CHAT_TEST_SEND`, `CHAT_TEST_DRAFT`,
  `CHAT_TEST_FOCUS`, `CHAT_TEST_SCROLLTO`, `CHAT_TEST_JUMP`, `CHAT_TEST_RESET`) to drive these from the command
  line: sending, the typing indicator and reply, persistence across relaunch, the keyboard, the jump button, dark
  mode and accessibility-XXXL.

## Not verified
- I couldn't trigger the long-press context menu (Copy and Delete) from simctl. It is the standard `.contextMenu`
  and wasn't exercised on the simulator. The same goes for swipe and scroll smoothness under a real finger.

## What would have made it faster
- A way to send taps and gestures to the simulator (something like idb or XCUITest) to check context menus and
  scrolling.
