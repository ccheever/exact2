# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift` — UIKit app/scene delegates (scene manifest in `EXTRA_PLIST`), Notes-yellow tint.
- `NoteStore.swift` — model + store. Seeds from the bundled `Resources/notes.json` (copy of `data/notes.json`) on
  first launch, then persists to Application Support as JSON, written synchronously on every change so nothing is
  lost if the app is killed mid-edit. Blank notes left over from a kill are purged on launch.
- `NotesListViewController.swift` — inset-grouped table with diffable data source; Pinned / Notes sections, newest
  first; search controller (case- and diacritic-insensitive, title or body, matches highlighted, body matches show a
  snippet of the matching line); toolbar with note count ("90 Notes", "3 of 90 Notes" while searching) and compose;
  leading swipe Pin/Unpin, trailing swipe Delete, plus a context menu (Pin, Share, Delete); empty and no-results
  states; relative dates (time today, Yesterday, weekday this week, short date otherwise) refresh on foreground.
- `NoteEditorViewController.swift` — title + body editor; the title lives in a header inside the body text view so
  they scroll together. Every keystroke saves and bumps `updated` (so the note moves to the top); no Save button.
  Return in the title jumps to the body. Done button while editing, "…" menu with Pin, Share, Delete (confirmed).
  A new note only enters the store once it has content; any note that is empty when you leave is discarded.
- Checked in light/dark and at the largest accessibility text size.

## What was hard / guessed
- No UI automation is available, so I tested with a temporary launch-argument script driver (open, focus, type,
  return, back, pin, delete, search) plus simctl screenshots and reading the app container's JSON; it was removed
  before the final build. Swipe gestures themselves were not exercised by touch, only the store actions behind them.
- `UITextView.insertText` (used by the driver) bypasses `shouldChangeTextIn`, which exposed that the title's
  Return handling needed a fallback in `textViewDidChange` too (also covers dictation/hardware keyboard paths).
- Guesses: an existing note emptied by the user is also discarded on leaving (like Apple Notes); search count shows
  "N of M Notes"; notes with an empty title use the first body line as their list title.
- The `-ResetNotes` launch argument wipes saved data back to the seed (handy for testing).

## What would have made it faster
- A way to send taps/swipes to the simulator (or XCUITest support in the skeleton) instead of a hand-rolled driver.
