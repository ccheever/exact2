# Chat — notes

SwiftUI only (`Sources/App.swift` views, `Sources/Store.swift` model/persistence). `Resources/messages.json` is the
shipped history (copied from `data/`).

## How it works
- `ChatStore` (@Observable) loads the bundled 520 messages, or the saved thread from
  `Application Support/thread.json` once anything has changed. Every send, Sam reply and delete rewrites that file,
  so sent messages (and deletions) survive relaunch.
- The thread is a `ScrollView` + `LazyVStack` of precomputed rows (day headings + bubbles with grouping flags),
  `defaultScrollAnchor(.bottom)` for the initial offset and for size changes (keyboard), and a `ScrollPosition`
  for programmatic scrolling. `onScrollGeometryChange` tracks distance from the bottom: the jump-to-newest button
  appears beyond 1.5 screen heights; Sam's replies auto-scroll only if you're already near the bottom.
- Composer is a `safeAreaInset` (so it rides the keyboard), multi-line, glass background; Send is disabled
  for whitespace-only text and text is trimmed. Interactive keyboard dismissal on scroll.
- Each send schedules a reply 2 s later (👍 weighted, or a canned line); a pending-reply counter drives the
  "Sam is typing…" row (animated dots) and the "typing…" subtitle in the nav bar.
- Long-press context menu: Copy / Delete (also exposed as VoiceOver actions). Day headings: Today / Yesterday /
  weekday / "Sat, Jul 4", plus the time of the day's first message, in the local time zone.

## What was hard / guesses
- Initialising `ScrollPosition(edge: .bottom)` shifted the content ~12pt horizontally and overscrolled past the
  bottom. Switching to an id-typed `ScrollPosition` and `scrollTo(id: "bottom", anchor: .bottom)` fixed it.
- There's no tap automation, so I added launch-argument hooks (`-debugSend <text>`, `-debugFocus YES`,
  `-debugTop YES`, `-debugJump YES`) to drive send, keyboard, scroll-to-first and jump-to-newest from
  `simctl launch`, and checked screenshots in light/dark and at AX5 text size.
- Fixed-size send/jump buttons overflowed at the largest text size; now `@ScaledMetric` with a cap.
- Guessed: canned reply wording, heading format, deleting is permanent (persisted), no undo.

## Not verified
- The long-press menu and tapping the jump button by hand (no way to tap from the CLI); the code paths behind
  them were exercised via the hooks.

## What would have helped
- A CLI tap/scroll tool for the simulator (or XCUITest via an Xcode project) to test gestures directly.
