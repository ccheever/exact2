# Notes: build notes

The app is built with Exact from the `list-detail` pattern and installed on simulator
`CEDFB9C9-…` as `Notes` (`dev.bakeoff.notes.b477b25`). `bun exact.mjs verify` and
`verify ios` both finish clean, with 7 tests passing on web and iOS. They check light and dark mode,
the largest accessibility text size, real-touch scrolling and a relaunch.

## What it does
- **List:** a large "Notes" title with the count under it ("90 Notes", or "3 of 90 Notes" while
  searching). A "Pinned" section comes first, then "Notes", each sorted most recently updated
  first. Each row shows the title, then the date in Mail's style (the time if today,
  Yesterday, a weekday, else 9/9/26) beside a one-line preview: the body's first non-empty
  line, cut off with an ellipsis.
- **Search:** UIKit's search field filters by title or body on every keystroke. When nothing
  matches, an empty state says so.
- **Editor:** tapping a row pushes an editor with a borderless title field and body text view.
  Every keystroke saves, with no Save button, and re-stamps `updated`, so the note moves to the
  top. Return in the title moves to the body. The bar has Pin and Delete buttons.
- **New note:** the compose button opens a blank editor with the keyboard up. Nothing is written
  until the note has text, so an empty new note is simply never saved. A note whose title and
  body are both cleared is removed too, as Apple Notes does.
- **From the list:** swipe a row to delete it (UIKit's own swipe action). Long-press a row for a
  menu with Pin Note / Unpin Note and Delete.
- **Persistence:** notes live in `app:/data/notes.json`. On first launch it is seeded from the
  shipped `data/notes.json`, imported by `app.ts` and so bundled into the app.

## What was hard / what I had to look up or guess
- **Time in the data module.** The data module can't read the clock, so the view passes
  `exactTime().epochAtZero + performanceNow()` into `saveNote`. The agent's frozen clock
  defaults to 2026-01-01, which is earlier than every shipped note. Early drives therefore made
  an edited note sink to the bottom. It was only the test clock, and `--epoch now` / `epoch` in
  tests fixed it.
- **Editor state on reload.** At first `follow` filled the drafts when a row was opened, so a
  web reload on `/note/:id` showed empty fields (verify caught it). I moved the editor into a
  child component whose state starts from the note, as the `edit-sheet` recipe does. It renders
  only after the notes have loaded.
- **iOS relaunch.** On iOS a relaunch opens at the list rather than restoring the open note, so
  a test expecting the editor after `reload` passed on web and failed on iOS. The test now
  reopens the note by its link.
- **Small syntax surprises:**
  - a block `match` takes no `=>`;
  - `font-weight` is a number;
  - `when` is a reserved word;
  - a section `header`/`footer` can't sit inside an `if`;
  - test `expect` takes only `==`.
- **Guesses:**
  - The count goes in the navigation bar's subtitle, not a bottom toolbar.
  - Pinning lives in the long-press menu and the editor's bar, not a leading swipe. A custom
    swipe would replace UIKit's native delete swipe.
  - `focus("body")` as the host command, which worked.
- **Accepted finding.** In the wide web window, the rows' hidden swipe-Delete buttons count as
  "covered". That finding is accepted in `app.json`'s `verify.accept`; it doesn't apply on
  iPhone.

## Not finished / limitations
- Each keystroke rewrites the whole JSON file (about 90 notes, fine at this size). A much larger
  library would want SQLite or debounced saves.
- There's no undo after a delete (the swipe deletes at once, as Notes does), and no Recently
  Deleted folder.
- The Linux and web targets were left in place but only lightly considered.

## What would have made this faster
- A "notes" or "editable document list" pattern: save-as-you-type, discard-if-empty, and pinned
  sections are a common shape.
- The agent defaulting to the real date, or warning when stored dates are later than its epoch.
- A note in the card that iOS `reload` doesn't restore the route while web does.
