# Notes app (UIKit) — build notes

## What's here
- `Sources/AppDelegate.swift`: app delegate and scene delegate. The scene manifest is in `EXTRA_PLIST` in `app.env`.
- `Sources/NoteStore.swift`: the `Note` model and the store. On first launch it loads the bundled `Resources/notes.json` (a copy of `data/notes.json`). After that it loads `Application Support/notes.json`. Every change is written straight to disk, so a note survives even if the app is killed while you're typing.
- `Sources/NotesListViewController.swift`: an inset-grouped table with a diffable data source.
  - "Pinned" section first, then "Notes", both sorted most recently updated first.
  - Search updates as you type. It matches title or body and ignores case and accents.
  - Swipe right to pin or unpin, swipe left to delete; a long-press menu offers both too.
  - A count in the toolbar ("90 Notes", or "3 of 90 Notes" while searching) and a compose button.
  - Empty states for "no notes" and "no results".
- `Sources/NoteEditorViewController.swift`: a title view and a body view, with no Save button.
  - Every keystroke saves and bumps `updated`, so the note moves to the top.
  - A new note is only added once it has content. It is deleted if it's empty when you leave the editor.
  - Return in the title moves to the body.
  - A "…" menu offers Pin/Unpin and Delete (with confirmation), and Done dismisses the keyboard.
  - The bottom of the body follows `keyboardLayoutGuide`.

## Tested on the simulator
Tested with screenshots via `simctl`:
- list, search, editor, new note with keyboard
- dark mode with the largest accessibility text size
- edit → moves to top and persists across relaunch
- new note typed → kept; new note typed then cleared → discarded
- pin, unpin and delete, checked in the saved JSON after relaunch

There is no tap or typing tool, so I drove these through a temporary environment-variable hook (`SIMCTL_CHILD_…`) that called the same code paths. I removed it before the final build. Swipe gestures and context menus are standard UIKit and were not exercised by hand. The final install was a clean reinstall, so the app starts with the 90 shipped notes.

## Decisions and guesses
- Dates: time if today, "Yesterday", the weekday within the last week, else a short numeric date.
- An existing note emptied by the user is kept (shown as "New Note"). The spec only says to discard empty *new* notes, and silently deleting existing data seemed worse.
- Pinning doesn't change `updated`; only edits to the text do.
- Accent colour is a Notes-like amber, adjusted for light and dark mode.

## What was hard / what would help
- Without a way to tap or type in the simulator, testing interactions needed a custom hook. A tap/type tool (or idb) would have made this faster.
- A debug-hook crash on an empty argument briefly looked like an app crash.
