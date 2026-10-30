# Notes (UIKit) — build notes

## What's here
- `Sources/AppDelegate.swift` — UIKit app + scene delegate (scene manifest in `app.env`'s `EXTRA_PLIST`).
- `Sources/NoteStore.swift` — `Note` model, JSON persistence in Application Support (seeded from the bundled
  `Resources/notes.json`, a copy of `data/notes.json`, on first launch), list date formatting.
- `Sources/NotesListViewController.swift` — inset-grouped table with a diffable data source: "Pinned" section
  first, then "Notes", each sorted most recently updated first. Search controller (title or body,
  case/diacritic-insensitive, as you type), swipe right to pin/unpin, swipe left to delete, long-press context
  menu with both, note count ("90 Notes", or "N Results" while searching) in the toolbar beside the compose
  button, empty states for no notes and no search results.
- `Sources/NoteEditorViewController.swift` — title (multi-line, Return jumps to the body) and body in one
  scrolling text view; every edit is saved immediately (disk write debounced 0.4 s, flushed on leave/background)
  and bumps `updated`, so the note moves to the top. Pin/Delete in the "…" menu, Done button while typing,
  keyboard handled with `keyboardLayoutGuide`. A new note is only created once it has text; a new note that is
  blank when you leave it is discarded (and any blank note found at launch is purged).
- `Sources/DebugScenario.swift` + `shot.sh` — test hooks: `simctl launch … -scenario "open:0;title:x;back"`
  drives screens without taps. Inert unless that launch argument is passed.

## What was hard / guessed
- No tap automation is available, so I tested by adding the launch-argument scenario hooks above and taking
  screenshots. Swipe actions and the context menu use stock UIKit APIs but weren't exercised by touch.
- Guessed the list date style to match Apple Notes: time today, "Yesterday", weekday within the week,
  otherwise `9/27/26`.
- The count while searching shows the number of matches; otherwise the total.
- Only *new* blank notes get discarded. An existing note you empty out is kept (it shows as "New Note"),
  so clearing a note's text can never silently delete it.
- On iOS 26, a label in the toolbar gets a glass capsule unless `hidesSharedBackground = true` is set.
- Header inside a scrolling `UITextView`: done by placing a subview in the content area and setting
  `textContainerInset.top` to its measured height, so the title scrolls with the body.

## Checked on the simulator
Light/dark, AX5 text size, search, editing + move to top, persistence across relaunch, blank-note
discard, new note creation, pin/unpin/delete.

## What would have made this faster
A way to send taps/swipes to the simulator (e.g. idb or XCUITest-runner support in the skeleton).
