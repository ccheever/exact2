# Chat — build notes

## What's here
- `Sources/ChatStore.swift`: an `@Observable` store. It loads `messages.json` from the bundle (copied into `Resources/`) on first launch. After that, the full thread (history, sent messages, Sam's replies and deletions) is saved as JSON in Application Support on every change and reloaded on launch. It also precomputes day headings and bubble grouping once per change rather than on every view render.
- `ThreadView.swift`: `ScrollView` + `LazyVStack` with `defaultScrollAnchor(.bottom)` (it opens at the newest message and stays pinned when the keyboard resizes the view), `scrollDismissesKeyboard(.interactively)`, and a jump-to-newest glass button that shows when you are more than one screen up (`onScrollGeometryChange`). The composer is in `safeAreaBar` so it rides above the keyboard and gets the iOS 26 scroll-edge blur.
- Sam answers each sent message 2 s later with a random canned reply (👍 is weighted). While a reply is pending, the thread shows a typing bubble labelled "Sam is typing…" and the nav title shows "typing…". The thread follows new messages only if you are near the bottom; sending always scrolls to the bottom.
- Long-press opens a context menu with Copy and Delete, and VoiceOver gets the same two actions. Whitespace-only drafts can't be sent (the send button is disabled and the text is trimmed). A message that is a single emoji is drawn large without a bubble.
- Day headings ("Today 9:54 PM", "Yesterday …", the weekday within the last week, then "Thu, Jul 16 …") use the local time zone. The data timestamps are UTC.

## Tested on the simulator
I checked the initial scroll to the bottom; sending, then the typing indicator, then the reply; that a sent message survives a relaunch; that the keyboard opens with the newest messages still visible; the jump button when scrolled far up; dark mode; and AX-XXXL text. I drove these with launch arguments (`-autoSend <text>`, `-focusComposer`, `-scrollToID <n>`, `-resetThread`) because simctl can't tap.

## Not verified / guesses
- I couldn't test the long-press menu or tapping the jump button on the simulator because there is no tap tool. Both use standard SwiftUI APIs (`contextMenu`, `ScrollViewProxy.scrollTo`), and `scrollTo` is the same call that the verified send path uses.
- The spec doesn't define "far up", so I chose more than one viewport height. It also doesn't list the canned replies, so I wrote my own.
- At first the composer's `safeAreaInset` got no scroll-edge blur and the overlay button overlapped the composer. I fixed this by switching to `safeAreaBar` and putting the button in an overlay on the composer.

## What would have made it faster
A way to send taps and long-presses to the simulator (e.g. idb or an XCUITest runner) so interaction tests didn't need launch-argument hooks.
