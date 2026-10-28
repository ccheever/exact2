# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift` — UIKit app/scene delegates (scene manifest in `EXTRA_PLIST`).
- `NoteStore.swift` — model + JSON persistence in Application Support, seeded from the bundled
  `Resources/notes.json` (copy of `data/notes.json`) on first launch. Saved on every change.
- `NotesListViewController.swift` — inset-grouped table with a diffable data source; "Pinned" and
  "Notes" sections, search controller (title/body, case- and diacritic-insensitive, live), swipe
  right to pin/unpin, swipe left to delete, long-press context menu for both, toolbar with note count
  ("90 Notes", or "N Results" while searching) and compose button, empty/no-results states.
- `NoteEditorViewController.swift` — title + body editor; saves on every keystroke (no Save button),
  bumps `updated` so the note moves to the top, Return in the title jumps to the body, Done button
  while editing, ⋯ menu with pin and delete. A new note is only added to the store once it has text,
  and it is removed again if it is empty when you leave.

## What was hard / guessed
- **A seed note dated in the future** (`2026-10-26T17:13Z`, later than "now" on the test day). Left
  alone, an edited note would sort *below* it, so "edits move the note to the top" would fail. On
  first import I clamp future dates to now, and edits also take `max(now, newest + 1s)` as a fallback.
- Diffable data source + section header titles: I first re-applied a snapshot from the apply completion
  to refresh headers, which crashes ("applying snapshots reentrantly"). I fixed it by reloading the
  surviving sections in the same snapshot.
- Largest accessibility text: the title is a non-scrolling `UITextView` (it wraps instead of
  truncating) inside a header that scrolls with the body (`textContainerInset.top` = header height).
  In list rows at accessibility sizes, the date and preview go on separate lines.
- I guessed that an *existing* note emptied by the user is kept (shown as "New Note"); the spec only
  says to discard empty *new* notes.

## Testing
No tap/gesture tool was available (`simctl` can't tap), so I drove the UI with a temporary
launch-environment script hook (create, type, return, back, search, open, pin). I checked the results
with screenshots and by reading the saved JSON in the app container, then deleted the hook. Checked:
seed import (90), new note → saved and at top, empty new note → discarded, edit → moves to top,
search, pin, relaunch persistence, dark mode, AX-XXXL text. I did not exercise swipe actions or
context menus by touch.

## What would have made this faster
A simulator tap/swipe tool (idb/XCUITest runner) for real gesture testing, and a fixed "now" or data
dated in the past.
