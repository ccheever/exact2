# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift` — UIKit app + scene delegate (scene manifest in `app.env` `EXTRA_PLIST`).
- `NoteStore.swift` — model + JSON persistence in Application Support. First launch seeds from the
  bundled `Resources/notes.json` (copy of `data/notes.json`). Pin/delete/create save immediately; typing
  is coalesced (0.4 s) and flushed when leaving the editor and when the app resigns active/backgrounds.
- `NotesListViewController.swift` — inset-grouped table with a diffable data source: "Pinned" and "Notes"
  sections, both sorted by most recently updated. Rows show the title, a date (time today, "Yesterday",
  weekday within a week, otherwise a short date) and a one-line preview. Search (title or body,
  case/diacritic-insensitive, live as you type), a toolbar note count ("90 Notes", or "3 of 90 Notes" while
  searching), a compose button, swipe actions (leading Pin/Unpin; trailing Delete + Pin), a long-press
  menu, and empty/no-results states.
- `NoteEditorViewController.swift` — title field + body text view, saved as you type with no Save button.
  An edit bumps `updated`, so the note moves to the top. A new note is only added to the store once it
  has content, and is removed if it is empty (whitespace only) when you leave. The editor also has a
  Done button while typing, an ••• menu (Pin/Unpin, Delete), an "edited" date, and constraints to the
  keyboard layout guide.

## Testing
I checked these with simulator screenshots: the list, the editor, a new note with the keyboard up, dark
mode, the largest accessibility text size (rows wrap to more lines), and loading saved data after a
relaunch (I edited the saved JSON and relaunched). `simctl` can't tap or type, so I did not test swipe
actions, search typing or live editing by hand. They are standard UIKit, and I read their code paths
closely instead of running them. There is a launch argument hook `-openNote <id|new>` so screenshots
can open the editor directly.

## Guesses / decisions
- Only *new* empty notes are discarded (that's what the spec says). An existing note that is cleared stays
  and shows as "New Note" / "No additional text".
- Section headers are always shown. Pinned notes are sorted by date inside their section.
- Deleting from the list asks for no confirmation, the same as Apple Notes.

## What would have made it faster
A way to send taps and text to the simulator (e.g. idb or XCUITest support in the skeleton) so I could
test the interactive flows end to end.
