# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift`: UIKit app/scene delegates (scene manifest in `EXTRA_PLIST`; `@objc(SceneDelegate)` gives the class a stable ObjC name, so it doesn't depend on swiftc's module name).
- `NoteStore.swift`: the model plus a JSON store in Application Support, seeded from the bundled `notes.json` on first launch. Saves are debounced by 0.5 s, and the store also saves when the scene resigns active or goes to the background.
- `NotesListViewController.swift`: an inset-grouped table with a diffable data source. It has "Pinned" and "Notes" sections, sorted by `updated` (newest first), with live search over title and body (ignores case and accents), and a toolbar showing the count ("N Notes", or "N Results" while searching) and a compose button. Swipe right to pin or unpin; swipe left to delete or pin. A long-press context menu offers both. There are empty and no-results states.
- `NoteEditorViewController.swift`: a title text view and a body text view in a scroll view sized to the keyboard layout guide. Every keystroke saves and sets `updated` to now, so the note moves to the top. Return in the title moves to the body. There's a Done button while editing and a ⋯ menu with Pin/Delete. A new note isn't stored until it has text, and a note that is empty when you leave it is discarded (Apple Notes does the same).

## Tested on the simulator
I used launch-argument hooks (`-test new|newEmpty|edit:<id>|open:<id>|search:<q>|pin:<id>|delete:<id>`, in `SceneDelegate.runTestScript`), because `simctl` can't tap or type. Verified: the seed list, creating a note, discarding an empty new note, an edit moving the note to the top, pinning, search, the no-results state, the data surviving a relaunch (checked the JSON in the container), dark mode, and the AX3 text size.
I couldn't drive real touches, so swipe actions, the context menu and keyboard behaviour were checked only by reading the code, not by hand.

## Guesses / decisions
- Dates: a time for today, "Yesterday", the weekday within the last week, otherwise a numeric short date.
- With no pinned notes, section headers are hidden.
- At accessibility text sizes, the date and preview go on separate lines.
- If the title is empty, the list uses the body's first line as the title.

## What would have made it faster
A way to send taps and keystrokes to the simulator (e.g. XCUITest or idb) for testing real interactions.
