# Chat — build notes

## What's here
- `Sources/ChatStore.swift`: model, seed loading (`Resources/messages.json`, copied from `data/`), persistence
  (whole thread as JSON in Application Support, so both sends and deletes survive relaunch), day/group row
  building, Sam's canned reply 2 s after every send (each send gets its own reply; "typing" shows while any are pending).
- `Sources/ChatView.swift`: `ScrollView` + `LazyVStack` with `defaultScrollAnchor(.bottom)` and `ScrollPosition`;
  composer in `safeAreaInset(.bottom)` (Liquid Glass) so it rides the keyboard; `onScrollGeometryChange` tracks
  distance from bottom to show the jump-to-newest button (with an unseen-reply badge) and to decide whether
  incoming replies should auto-scroll. Long-press context menu: Copy / Delete (also exposed as VoiceOver actions).
- Return sends (vertical TextField can't submit, so a typed trailing newline is intercepted); whitespace-only
  text can't be sent and the send button is disabled.

## Tested on the simulator
Opens at newest; send + typing indicator + reply; relaunch persistence; scroll to first message; jump button;
keyboard open keeps the newest messages visible; dark mode + AX3 text size.
Testing used launch arguments (`-resetThread`, `-autoSend <text>`, `-focusComposer`, `-scrollTo top|<id>`) since
simctl can't tap or swipe. They are harmless and left in.

## Hard / guessed / unfinished
- Couldn't exercise long-press or finger-scroll smoothness from simctl (no touch injection); context menu and
  scroll feel are verified only by code, not by hand.
- Guessed: grouping gap (15 min), heading format (iMessage-style "Sat, Jul 4 10:20 AM", Today/Yesterday/weekday),
  canned reply list, delete without confirmation.
- `Date.ISO8601FormatStyle` vs `.iso8601.currentTimestamp` API naming cost a build cycle.
- Pending replies are not persisted if the app is killed during the 2 s window.

## What would have helped
A way to inject taps/swipes/long-presses into the simulator (or an XCTest UI runner) to test gestures directly.
