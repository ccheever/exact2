# Notes app — build notes

UIKit, programmatic. Files: `App.swift` (app + scene delegate), `NoteStore.swift` (model, JSON persistence,
date formatting), `NotesListViewController.swift` (list/search/pin/delete/count), `NoteEditorViewController.swift`.

## Behaviour decisions (guesses where the spec was silent)
- Seed `notes.json` is bundled; on first launch it is copied to Application Support and that copy is used from then on.
  Saves are debounced (0.5 s) and flushed when the editor closes and when the app resigns active/backgrounds.
- The list is ordered by `updated` (newest first), tie-broken by id. Pinned notes get their own "Pinned" section.
  Section headers are shown only when there are pinned notes.
- Date: time for today, "Yesterday", weekday within the last week, otherwise a short date.
- Count is in the bottom toolbar ("90 Notes"). While searching it shows "N of M Notes".
- Search matches title or body, case- and diacritic-insensitive (`localizedStandardContains`), live as you type,
  and keeps the pinned/notes sections. Shows the system "No Results" state when nothing matches.
- Editing sets `updated` only when the text actually changes, so just opening a note doesn't reorder it.
- A new note isn't added to the store until it has some text. Any note that is empty when you leave it is deleted
  (as Apple Notes does), and empty notes left by a mid-edit kill are purged on load.
- Pin/unpin: leading swipe or context menu. Delete: trailing swipe (full swipe works) or context menu. The editor's
  "…" menu also has Pin and Delete. No delete confirmation (matches Notes' swipe behaviour); there's no undo either.
- Title is a single logical line: Return moves to the body, and pasted newlines become spaces.

## What was hard
- Can't tap the simulator with only `simctl`, so I tested editing, creating, discarding and persistence with
  temporary launch-argument hooks (since removed). I checked the results in screenshots and in the saved JSON.
- In the editor, the title, date and body sit in one scroll view (so the title scrolls away), with the bottom pinned to
  `keyboardLayoutGuide`. That meant scrolling to the caret by hand, and getting the content-hugging priorities right
  so the date label didn't stretch.

## Not done
- No undo for delete and no "Recently Deleted" folder. No multi-select editing.
- The toolbar count label is a custom view and stays fairly small at accessibility text sizes.

## Would have made it faster
- A way to send taps and text to the simulator (e.g. an idb-style tool), or XCTest UI tests.
