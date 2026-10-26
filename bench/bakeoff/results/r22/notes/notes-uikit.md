# Notes (UIKit) — build notes

Programmatic UIKit app: `AppDelegate`/`SceneDelegate` (scene manifest in `app.env`'s `EXTRA_PLIST`),
`NoteStore` (JSON in Application Support, seeded from the bundled `Resources/notes.json` = `data/notes.json`),
`NotesListViewController` (inset-grouped table, diffable data source, Pinned/Notes sections, search,
swipe + context-menu pin/delete, count in the iOS 26 nav-bar subtitle) and `NoteEditorViewController`
(autosaving title + body; an empty new note is never stored and is discarded on leaving).

## What was hard
- Making the editor work at the largest accessibility text size: a single-line `UITextField` truncated the title,
  and a fixed header above the body ate the screen. Solved by making the title a non-scrolling `UITextView` and putting
  the date + title header *inside* the body text view (offset via `textContainerInset.top`) so the whole note scrolls.
- No UI-automation tool is available, so I added an env-gated hook (`NOTES_DEBUG`, passed with
  `SIMCTL_CHILD_NOTES_DEBUG`) that scripts open/new/type/back/search/pin/delete, then checked screenshots and the
  saved JSON. It does nothing unless that variable is set.

## Decisions / guesses
- Dates: time for today, "Yesterday", weekday within the last week, otherwise a short numeric date.
- Ties in `updated` sort by id. Pinned notes are sorted by recency within their section too.
- An untitled note with a body shows its first body line as the list title (as Apple Notes does).
- Only *new* empty notes are discarded; clearing an existing note keeps it (it's never silently deleted).
- Deletes from the list happen right away, with no confirmation, as in Apple Notes swipe-to-delete.
- Search matches title or body, ignoring case and diacritics; while searching the subtitle shows the result count.
- Used iOS 26 APIs (`navigationItem.subtitle`, search-bar toolbar integration) because the minimum OS is 26.

## Not done
- No undo for delete, no rich text, no sharing, no state restoration of the open note across relaunch.

## What would have made it faster
- A UI-automation tool for tapping and typing in the simulator (or XCUITest support in the build script).
