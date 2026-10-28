# Chat — notes

SwiftUI app, `Sources/` (App.swift, Model.swift, ThreadView.swift). `data/messages.json` is shipped in `Resources/`.

## How it works
- `ChatStore` (@Observable) loads the bundled history on first launch, then saves the whole thread (sends, Sam's
  replies and deletions) to Application Support/thread.json after every change; later launches read that file.
- Thread: `ScrollView` + `LazyVStack` with `ScrollPosition(edge: .bottom)` and `.defaultScrollAnchor(.bottom)` so it
  opens on the newest message and stays bottom-pinned when the keyboard or the typing indicator changes the size.
  Composer is a `.safeAreaInset(edge: .bottom)`, so it rides above the keyboard; interactive keyboard dismissal.
- Date headings per local calendar day (Today / Yesterday / weekday within a week / "Wed, Jul 1", year if not this year).
  Consecutive same-sender messages within 5 minutes are grouped with tighter spacing and flattened corners.
- Send: trimmed text, the button is disabled for empty/whitespace drafts, and the return key (labelled Send) sends too.
  Each send schedules one Sam reply 2 s later (👍 or a canned line); "Sam is typing…" with animated dots shows while any are pending.
- Long-press context menu: Copy (UIPasteboard) and Delete (also exposed as VoiceOver actions).
- A floating glass ↓ button shows when you are more than ~900 pt from the bottom and scrolls back.
- Dark mode uses system colours; checked at the largest accessibility text size.

## Testing
No tap/touch driving tools were available (no idb; AppleScript to Simulator timed out), so I added a simulator-only
hook: `SIMCTL_CHILD_CHAT_DEMO=keyboard,type,send ./…` (see `shot.sh`) runs steps in the app and I screenshot them.
Verified: opening at newest, scroll to top (first message, Wed Jul 1), jump button, keyboard + composer, send,
typing indicator, reply, persistence across relaunch, dark mode + AX3 text. Not verified by touch: the long-press menu
and finger-flick scrolling smoothness (couldn't inject gestures). The installed app was reset to a fresh state.

## Guesses / limits
- Many history messages share a timestamp; ordering keeps file order (sorted by date, then id).
- If the app is killed during the 2-second wait, Sam's pending reply is lost (not persisted).
- Deleting history messages is persisted too (the saved thread replaces the bundled one).

## What would have made it faster
A way to send taps/long-presses/swipes to the simulator (idb or similar) to test gestures directly.
