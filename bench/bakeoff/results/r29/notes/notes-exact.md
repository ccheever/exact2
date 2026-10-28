# Notes (Exact) — build notes

## What was built
- `app.contract`: a list screen (large title "Notes", "N Notes" subtitle, compose bar button, bar search
  field) with a **Pinned** section above a **Notes** section, both sorted most recently updated first. Each row
  has the title, then Notes' list date (time today / Yesterday / weekday / 10/11/26) and the body's first line.
  Rows swipe to delete. A long press opens a menu with Pin Note / Unpin Note and Delete. A row pushes the note
  editor. The editor has the date, a borderless title field and a body text view. Every keystroke saves (no Save
  button) and stamps the note with the current time, which moves it to the top. The nav bar has a pin toggle,
  and Return in the title moves to the body.
- `app.ts`: the notes live in `app:/data/notes.json`. On first launch they come from the shipped
  `data/notes.json`, which is imported into the bundle. `saveNote` adds or updates a note. `leaveNote` discards
  a note that is empty when you leave it. At launch, empty notes are dropped too, in case the app was killed on a
  blank new note.
- A new note doesn't touch storage until you type into it. Its id is the creation timestamp.
- Tests (`app.test.contract`, 5): seed, sections and count; search; edit, move to top and relaunch; new note
  kept or discarded; pin, unpin and delete, and relaunch. `verify` and `verify ios` are both clean: light and
  dark, the largest text size, and the scrolling check.

## What was hard / what I had to find out
- On the web, Back goes through the `navigate=` action, not `navigationBack`. So the empty-note discard has to
  run in both `follow` and `back`, and my first test failed until I added it to `follow`.
- A `header` only becomes the iOS nav bar when it is a direct child of the route's column. Inside a child
  component it rendered as content, so the pin button moved up a level.
- A section `header` can't sit inside an `if`. I had to use two `section` variants instead.
- At the largest text size, verify flagged the date and preview being side by side. I fixed it by letting the
  preview wrap onto its own line (`flex-wrap`, with `flex-basis` set to 8rem) while keeping it to one line.
- The test and agent clock defaults to 2026-01-01, which is older than the shipped notes. So a fresh edit
  sorted to the bottom until I pinned `epoch` in the tests. The real app uses real time.
- The data module isn't allowed to read the clock, so the view stamps the time
  (`time.epochAtZero + performanceNow()`) and passes it to the write.

## Guesses
- I put the count in the large title's subtitle ("90 Notes"). It shows the total, not the number of search
  matches.
- Pinning from the list is in the long-press menu (as in Apple Notes), not a leading swipe. Exact's leading
  swipe needs a hand-built scroll row.
- There is no pin-slash symbol in Exact's set, so the editor's pin button uses `symbol:pin` with
  `aria-pressed`.

## Not done
- There is no "Done" button to dismiss the keyboard in the editor. Dragging the scroll dismisses it
  (`keyboardDismissMode="interactive"`).
- I didn't build a multi-column layout. The app is iPhone-only.

## What would have made this faster
- A "notes / editor that saves as you type" pattern, and a clear note in the `screen` vocab that Back on the
  web arrives through `navigate` and not `navigationBack`.
- A way to set the agent's epoch, so drives show realistic dates.
