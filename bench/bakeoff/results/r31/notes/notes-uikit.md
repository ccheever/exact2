# Notes (UIKit) — build notes

## What's here
- `Sources/AppDelegate.swift`: UIKit app and scene delegates. The scene manifest is in `EXTRA_PLIST` in `app.env`.
- `Sources/NoteStore.swift`: `Note` model and `NoteStore`. On first launch the store seeds itself from the bundled
  `Resources/notes.json`, a copy of `data/notes.json`. It saves JSON to Application Support about 0.4 s after a change
  (debounced), and also saves at once when the app resigns active, goes to the background or terminates.
  The date helpers live here too: the list shows the time for today, "Yesterday", the weekday within the last week,
  and a short numeric date otherwise.
- `Sources/NotesListViewController.swift`: inset-grouped table with a diffable data source, a "Pinned" section
  first, and live search on title or body (case- and diacritic-insensitive). The toolbar shows the note count
  ("N Notes", or "N Found" while searching) and a compose button. Swipe right to pin or unpin, swipe left to
  delete; a long-press context menu offers the same actions. There are empty states for "No Notes" and for a search
  with no results. Dates refresh when the day changes and when the app returns to the foreground.
- `Sources/NoteEditorViewController.swift`: a title and a body, each a self-sizing `UITextView` inside one scroll
  view, pinned to `keyboardLayoutGuide`. Every keystroke saves, and a real change stamps `updated`, which moves
  the note to the top. Return in the title moves to the body. A Done button appears while you type, and a "…" menu
  offers Pin/Unpin and Delete. A new note is only created once it has text. Leaving a note that is empty (new or
  emptied) deletes it.

## Tested on the simulator
I couldn't tap on the simulator, so I used a temporary env-var-driven test driver (since removed) to open notes,
type, pop, pin, delete and search, and checked the result with `simctl` screenshots and the saved JSON:
- An edit moves the note to the top with today's time.
- An empty new note is discarded (the count stays at 90).
- A new note with text is kept.
- Pin and delete work, and both survive a relaunch.
- The no-results empty state shows.
- Dark mode and AX-XXXL text both look right.
- The caret stays visible above the keyboard in a long note.

Not tested by real touch: the swipe gestures, the context menu and the Done/… buttons. They use standard UIKit API.

## Guesses and decisions
- Pinning does not change `updated`, so a note keeps its place in date order (Apple Notes does the same).
- A note whose title is empty shows as "New Note" in the list. One with an empty body shows "No additional text".
- At accessibility text sizes the date gets its own line, so the preview still has a full line.
- On load, the store drops notes that are completely empty, which can happen if the app is killed while a note is
  emptied.

## What was hard / what would have helped
- The main obstacle was having no way to tap on the simulator: an idb/XCUITest-style tap tool, or a ready-made
  launch-argument test hook, would have saved writing a throwaway driver.
- Diffable data sources have no built-in way to update a section header, so the "Notes" header (shown only when
  there are pinned notes) needs `reloadSections` when the Pinned section appears or disappears.
- Nothing was left unfinished.
