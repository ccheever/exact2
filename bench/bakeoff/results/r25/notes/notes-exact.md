# Notes (Exact) — build notes

Built with Exact from the `list-detail` pattern. Bundle id `dev.bakeoff.notes.bbdec6a`, name `Notes`.
It's installed on simulator EEFC0029-1010-45E4-8242-42CF67E16C96. `verify` (web) and `verify ios` are both clean:
5/5 tests pass, light and dark, the largest text size, scrolling and relaunch.

## What's in it
- **List:** a "Pinned" section first, then "Notes", each sorted most recently updated first. A row shows the title
  (or the body's first line if there's no title), a Mail/Notes-style date (the time today, then Yesterday, the weekday, then m/d/yy)
  and a one-line preview. The note count ("90 Notes") is the large title's subtitle.
- **Search:** the bar's search field filters by title or body on each keystroke. No match shows an empty state.
- **Editor:** a pushed screen with a borderless large title field and a body text view. Every keystroke is saved
  through a queued write and stamps `updated`, so the note moves to the top. There's no Save button. Return in the title
  moves to the body, and a Done bar button hides the keyboard while you type.
- **New note:** the compose button in the bar. The note is only written once something is typed. Leaving a note
  that has no title and no body deletes it (`discardIfEmpty`, run in the same write queue after the last save, so the order is safe).
- **Pin/Unpin and Delete:** long-press a row for a menu (Pin Note / Unpin Note, Delete). Swipe a row to delete.
- **Persistence:** `app:/data/notes.json` is seeded from the bundled `data/notes.json` (imported by `app.ts`) on first
  launch. ISO dates become epoch ms.

## What was hard, looked up or guessed
- **Time:** the data module may not read the clock, so the view passes `time.epochAtZero + performanceNow()` to each save
  (from the `dates-and-days` recipe). A 30 s task ticks the relative dates.
- **Discard on leave:** it has to run on the bar's Back, on edge-swipe back, and on browser history (`follow`). I had to
  read the action-semantics note: writes land when the action ends, so I bind `let left = editing` first.
- **Tests:** the default test epoch is 2026-01-01, which is earlier than the data, so a "new note goes to the top" test failed
  until I pinned `epoch`. I also guessed `tap "back"` for the back button; the right test step is plain `back`.
- **Guesses:** an *existing* note emptied and left is deleted too, as Apple Notes does. Pinning doesn't change
  `updated`. Pin is in the long-press menu rather than a leading swipe, because the custom swipe form would replace UIKit's
  native delete swipe.
- **Grouped list:** a section header can't sit under an `if`, so there are two `section` branches for "Notes" with and without a header.

## Not finished / caveats
- Pin isn't offered as a swipe action, only in the long-press menu.
- The agent's iOS drives run on a frozen test clock, so screenshots from them show "Today at 12:00 AM". The installed app uses real time.

## What would have made it faster
- A "notes"/document-editor pattern: a borderless title + body editor that autosaves and discards when empty.
- Saying in the card that the test step for the system Back is `back`, next to the `navigationBack` example.
