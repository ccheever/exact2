# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift`: UIKit app and scene delegate. The scene manifest is in `EXTRA_PLIST`. Saves on resign-active and on background.
- `NoteStore.swift`: the `Note` model and a store that keeps notes in memory and writes JSON to
  `Application Support/notes.json`, 0.4 s after a change and immediately when you leave a note or the app.
  On first launch it seeds from the bundled `Resources/notes.json` (a copy of `data/notes.json`).
- `NotesListViewController.swift`: inset-grouped table with a diffable data source. Pinned and Notes sections,
  most recently updated first. Each row shows the title, then the date and a one-line preview.
  - Dates: the time for today, "Yesterday", the weekday within the last week, otherwise a short date.
  - Search is live, matches title or body, and ignores case and diacritics.
  - Swipe left to pin or unpin; swipe right to delete or pin; there is also a long-press context menu.
  - The toolbar shows the count ("90 Notes", or "N Found" while searching) and the compose button.
  - Empty states use `UIContentUnavailableConfiguration`.
- `NoteEditorViewController.swift`: a multi-line title and a body, both non-scrolling text views inside a
  scroll view pinned to `keyboardLayoutGuide`.
  - Every keystroke saves and bumps `updated`, so the note moves to the top. Just opening a note doesn't.
  - Return in the title moves to the body.
  - A Done button appears while editing; a ⋯ menu offers Pin/Unpin and Delete (with confirmation).
  - A new note isn't added to the store until it has content. If it is blank when you leave, it is
    discarded, and the count never includes it.

## Testing
I can't tap through `simctl`, so I added launch-argument hooks: `-search q`, `-openNote id`, `-newNote YES`,
`-typeTitle`, `-typeText`, `-popAfter YES` and `-resetNotes YES`. With these I checked editing, the
move-to-top, discarding an empty new note, creating a note, and that changes survive relaunch, by reading the
saved JSON and taking screenshots. I also checked light and dark mode at AX5 text size.
Swipe and context-menu pin/delete were not exercised by touch; they call the same store methods.

## Guesses / decisions
- An existing note that is edited until it's empty is kept, shown as "New Note". The spec only requires
  discarding empty *new* notes.
- A note without a title shows the first line of its body as the title, as Apple Notes does.
- At accessibility text sizes the row allows up to 3 lines for the title and 3 for the date + preview.

## What would have made it faster
A UI-automation tool for the simulator (taps and swipes) to test gestures directly, rather than through hooks.
