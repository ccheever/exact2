# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift`: UIKit app and scene delegates (scene manifest is in `EXTRA_PLIST` in `app.env`; the scene delegate is `@objc(SceneDelegate)` so the plist name doesn't depend on the swiftc module name).
- `NoteStore.swift`: the model and JSON storage. On first launch it seeds from the bundled `notes.json` (copied from `data/` into `Resources/`). After that it reads and writes `Application Support/notes.json`. Saves are debounced by 0.5 s and flushed when the app goes to the background and when you leave the editor.
- `NotesListViewController.swift`: an inset-grouped table backed by a diffable data source, with "Pinned" and "Notes" sections, most recently updated first.
  - Each row shows the title, then a date (time if today, "Yesterday", the weekday if within the past week, otherwise a short date) and a one-line preview of the body.
  - Search is a `UISearchController` that filters by title or body as you type, ignoring case and accents.
  - Swipe right to pin or unpin, swipe left to delete; a long-press menu offers the same actions.
  - The toolbar shows the note count ("91 Notes", or "N Results" while searching) next to a compose button.
  - Empty states (no notes, no search results) use `UIContentUnavailableConfiguration`.
- `NoteEditorViewController.swift`:
  - Title and body are separate text views. The header (date and title) sits inside the body's scroll view so everything scrolls together. Return in the title moves the cursor to the body.
  - Every keystroke saves and sets `updated = now`, so the note moves to the top of the list. Opening a note without changing it leaves its date alone.
  - A Done button appears while you're typing. A ⋯ menu offers Pin/Unpin and Delete (with confirmation).
  - The body stays above the keyboard via `keyboardLayoutGuide`; dragging down dismisses the keyboard.
  - A new note goes into the store only once it has some text. If it's empty when you leave, it is discarded.

## Tested on the simulator
- Launch screen with 90 notes and the pinned section.
- Editing a title saves it to disk.
- A new note with text persists across a relaunch (count went to 91).
- An empty new note is discarded.
- Search results in dark mode.
- The editor with the keyboard up, and the list, at the largest accessibility text size (rows wrap to up to 3 title lines and 2 preview lines).

simctl can't tap, so I drove these flows with a temporary launch-argument hook file, which has since been deleted. Swipe actions and context menus were therefore never exercised by touch; they use standard UIKit APIs.

## Guesses / decisions
- Only *new* empty notes are discarded, as the spec says. An existing note that you clear out is kept.
- Pinning doesn't change `updated`, so pinning doesn't reorder notes by date.
- On load, notes with an empty title and body are dropped. This cleans up after a crash in the middle of a new note.
- While searching, the count shows the number of matches; otherwise it shows the total.

## What was hard / would have helped
- With no way to tap the simulator (only simctl), interactive testing meant writing temporary launch-argument hooks. A tap/type tool (or XCUITest support in the build script) would have made testing quicker and more thorough.
- iOS 26 gives toolbar items glass backgrounds; the count label needed `hidesSharedBackground = true` to look like a plain caption.
