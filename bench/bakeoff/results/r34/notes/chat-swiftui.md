# Chat — notes

## Structure
- `Sources/Store.swift` — `ChatStore` (@Observable): loads history, sends, deletes, Sam's delayed replies, and precomputed thread rows (day headings and bubble grouping).
- `Sources/ChatView.swift` — the thread (`ScrollView` + `LazyVStack` + `ScrollPosition`), the jump-to-newest button, the nav title, and day headings.
- `Sources/Bubbles.swift` — message bubble (with the context menu), the typing indicator, and the composer.
- `Resources/messages.json` — a copy of `data/messages.json`, shipped in the bundle as the starting history.

## Behaviour decisions
- **Persistence:** the whole thread is saved as JSON to Application Support after every change (send, Sam's reply, delete). On first launch it loads the bundled history. Because of this, deleted history messages stay deleted too.
- **Date headings:** "Today", "Yesterday", the weekday name within the last week, then "Wednesday, July 1". The year is added only for other years. Days use the device's time zone.
- **Sam's replies:** each send schedules its own reply 2 s later: "👍" (weighted) or a canned phrase. While any reply is pending, a "Sam is typing…" bubble shows at the bottom and "typing…" shows under Sam's name. If the app is killed during those 2 s, the reply is lost; I judged that acceptable.
- **Following new messages:** sending always scrolls to the bottom. Sam's reply and the typing indicator only scroll the thread if you are already near the bottom, so reading old history isn't interrupted.
- **Jump button:** a glass ↓ button appears once you are more than about 600 pt from the bottom.
- **Sending:** blank or whitespace-only text can't be sent (the button is disabled and `send` also guards). Text is trimmed before sending. Return inserts a newline, as in Messages.
- **Emoji-only messages** (1–3 emoji) render large with no bubble.
- **Keyboard:** `defaultScrollAnchor(.bottom, for: .sizeChanges)` keeps the newest messages visible when the keyboard opens. The composer sits in a `safeAreaBar`, and the keyboard can be dismissed interactively by scrolling.

## Testing
I tested on the simulator with screenshots. The app has launch-argument hooks (`-send "text"`, `-focus`, `-top`, `-scrollUp <id>`) because there is no tap automation without XCTest. Verified:
- opens at the newest message
- sending, the typing indicator and Sam's reply
- the message survives a relaunch
- the keyboard keeps the newest messages visible
- scrolling to the very first message, with its day heading
- the jump button
- dark mode, and the AX5 (largest) text size

**Not verified by hand:** long-press Copy/Delete and tapping the jump button. Both use standard SwiftUI (`.contextMenu`, `Button`), but simctl cannot long-press or tap.

## What was hard / guessed
- Keeping a lazy SwiftUI stack anchored to the bottom without jumps. iOS 18+ `ScrollPosition` plus `defaultScrollAnchor` handles it. I scroll to a sentinel view at the very bottom instead of the last message, so the typing row is included.
- Content showing through the iOS 26 glass composer hurt legibility, so I added a soft fade behind it.
- Canned reply wording, the grouping gap (10 min) and the threshold for showing the jump button were guesses.

## What would have made this faster
A way to send taps or long-presses to the simulator (e.g. idb, or an XCUITest runner) would have let me verify the context menu and buttons directly.
