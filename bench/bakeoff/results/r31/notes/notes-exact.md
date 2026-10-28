# Notes (Exact) — build notes

Built from the `list-detail` pattern. `app.contract` (view), `app.ts` (data), `app.test.contract`
(7 tests). `bun exact.mjs verify` and `verify ios` pass: tests, light/dark, the largest text size
(nothing spills) and the scrolling check ("like UIKit"). The final build is installed on simulator
C2188CC6-… as `dev.bakeoff.notes.b3b7580` / "Notes".

## How the features are done
- **Data:** `app.ts` imports `data/notes.json` (bundled into the app) and on first launch uses it
  as the starting notes. After that it reads and writes `app:/data/notes.json`, so edits, pins and
  deletes survive a relaunch. Notes come back sorted newest first, and the view splits off a
  "Pinned" section (with a "Notes" header under it only when pinned notes exist).
- **Rows:** the title, then a Notes-style date (`formatDay(..., "list")`: the time today,
  Yesterday, the weekday, or 9/27/26) and the body's first line, cut off with an ellipsis. A note
  with no title is headed by the first line of its body, as Apple Notes does.
- **Count:** the large title's subtitle reads "90 Notes", or "19 of 90 Notes" while searching.
- **Search:** the bar's search field filters by title or body as you type, without regard to
  case, and shows an empty state when nothing matches.
- **Editing:** tapping a row opens an editor with a borderless title field and a body text view.
  Each keystroke sends a queued save, and the save stamps `updated` with the current time, so
  the note moves to the top. There is no Save button.
- **New note:** the compose button opens the editor with a fresh id. The note is only written on
  its first keystroke. Leaving through Back or the edge swipe (`navigationBack`) deletes it if its
  title and body are empty. An existing note that is emptied and left is discarded the same way,
  as Apple Notes does.
- **Pin and delete from the list:** swipe left to delete (UIKit's swipe action). A long press
  opens a menu with Pin Note / Unpin Note and Delete. The editor's bar also has Pin and Delete;
  that Delete asks for confirmation first.

## What was hard / what I guessed
- **The editor scrolled under the bar:** my first version put the title and a tall textarea
  (`min-height="60vh"`) inside a `scroll`. On iOS, typing pushed the caret up under the
  navigation bar and the fields seemed to disappear. Making the textarea `flex=1` and letting it
  scroll on its own fixed it. I only found this by taking screenshots of an agent drive.
- **Pin as a leading swipe:** Exact's "long form" swipe actions need the row rebuilt as a
  horizontal snap scroll, so I put Pin in the long-press menu instead. A leading Pin swipe would be
  more like Apple Notes.
- **Compile errors that pointed the way:** calling an action needs `leave()`, `font-weight` takes
  a number, and a section `header` cannot sit inside an `if`.
- **Symbols:** `pin.slash` has no cross-platform role, so I used an SF Symbol. That leaves two
  warnings on the web build, which this app does not ship as its product.
- **Clock in drives:** in an agent drive the editor's date showed "12:00 AM" because drives use a
  virtual clock. On the real launch, dates come from the device's time.

## Not done / could be better
- No leading swipe for Pin (it is in the long-press menu and the editor instead).
- Return in the title field does not move the cursor into the body.
- There is no Done button to dismiss the keyboard. Back dismisses it, and so does the
  system's own keyboard gesture.

## What would have made it faster
- A pattern for a document-style editor: a full-screen body text view that saves as you type, with
  discard-if-empty on leave. This is the most common "notes" shape, and the textarea-in-a-scroll
  trap is easy to fall into.
- A one-attribute leading swipe action (`swipeLeading=`) on an ordinary row, like `delete=`.
- An `expect` step in `agent` drives (it only has `state`), so a drive could check itself.
