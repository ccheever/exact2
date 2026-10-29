# Chat — notes

SwiftUI only (`UIPasteboard` is the one UIKit call). Files: `Sources/ChatStore.swift` (model, persistence,
Sam's replies), `Sources/ChatView.swift` (thread, bubbles, composer, typing indicator), `Sources/App.swift`.
`Resources/messages.json` is a copy of `data/messages.json`, shipped as the history.

## How it meets the spec
- **Thread:** `ScrollView` + `LazyVStack`, with `defaultScrollAnchor(.bottom)` so it opens on the newest message
  and stays pinned as content or keyboard size changes. A heading (Today / Yesterday / weekday / date, plus time)
  appears where the local day changes. Messages from the same sender within 5 minutes form a group with tighter corners.
  A message of only 1–3 emoji is shown large with no bubble, as in Messages.
- **Composer:** `safeAreaBar` (iOS 26), so it rides above the keyboard and gets the soft scroll-edge effect. It is a
  multi-line field (up to 6 lines). The send button is disabled when the trimmed text is empty. Sending clears the
  field and scrolls to the bottom. `scrollDismissesKeyboard(.interactively)` lets you drag the keyboard away.
- **Sam:** each message you send schedules its own reply 2 s later: "👍" (weighted) or a canned line. While any
  reply is pending, a dots bubble with "Sam is typing…" sits at the end of the thread and "typing…" shows in the
  nav title. New replies auto-scroll only if you are already at the bottom.
- **Long-press:** `contextMenu` with Copy and Delete (destructive); the preview is clipped to the bubble shape.
  VoiceOver also gets these as custom actions.
- **Jump button:** appears when you are more than a screen-height above the bottom.
- **Persistence:** the full thread (history + sent + replies + deletions) is saved atomically to Application
  Support/`thread.json` after every change and loaded from there at launch. The bundle seeds the first launch.

## What was hard / guessed
- **Measuring distance from the bottom with the keyboard up.** `contentOffset + containerSize + contentInsets.bottom`
  double-counts the keyboard during the transition, so the jump button showed while at the bottom. Logging
  `ScrollGeometry` showed that `contentSize.height - (visibleRect.maxY - contentInsets.bottom)` stays consistent.
  I worked this out from those logs; I found no documentation for it.
- **No way to tap or type in the simulator** with the tools provided. I added launch-argument hooks
  (`-uitestFocus`, `-uitestSend <text>`, `-uitestScrollTop`, `-uitestScrollTo <id>`), which do nothing in normal use.
  With them and `simctl io screenshot` I tested sending, the typing indicator, replies, the keyboard, relaunch
  persistence, scrolling to the first message, dark mode and AX5 text.
- **Long-press Copy/Delete was not tested on the simulator** (I can't long-press via `simctl`). It is the stock `contextMenu`.
- Guesses: the canned reply list; what counts as a group (5 min); the jump-button threshold (one screen); the send
  button capped at 44 pt at accessibility sizes so the text field keeps room; a pending reply is lost if
  the app is killed during its 2 s delay.

## What would have made this faster
A way to send touches or text to the simulator (e.g. idb or an XCUITest runner). Clearer documentation of
`ScrollGeometry` fields while the keyboard is moving.
