# Notes (Exact) — build notes

## What was built
- `app.contract`: the list (large title "Notes", a count subtitle that reads "N Found" while
  searching, a compose bar button, the bar's search field). Pinned notes are in a "Pinned"
  section above "Notes", and each section is sorted by `updated`, newest first. Each row shows
  the title, then the date (`formatDay … "list"`: the time today, Yesterday, the weekday, else
  M/D/YY) and the first non-empty line of the body.
  Rows swipe to delete (UIKit's swipe) and long-press for a menu with Pin/Unpin Note and Delete.
- Editor (pushed screen): the edited date, a borderless title field and a body textarea. Every
  keystroke sends a queued `saveNote` with the current time, so the note saves without a
  Save button and moves to the top. There's also a Pin button in the bar.
- New note: its id comes from the clock and nothing is written until it has text. On Back, a
  note with an empty title and body is removed, which covers both an empty new note and a note
  you cleared out. `loadNotes` also hides empty notes, in case the app is killed mid-edit.
- `app.ts`: the 90 notes in `data/notes.json` are imported as the seed. The first write
  stores everything in `app:/data/notes-store.json`, which survives relaunches.
- `app.test.contract`: 7 tests covering ordering, sections, dates, count, search, editing
  that moves a note to the top, discarding empty notes, pin/unpin, swipe and menu delete,
  and relaunches.

## Status
- `verify` (web): clean.
- `verify ios`: all 7 tests pass. The light/dark checks and the largest-text check (nothing
  spills) are clean. There are 3 remaining "scrolling against UIKit" findings: tracking ratio,
  decelerating jumps, and status-bar scroll-to-top time. I have no scroll handler and the rows
  are two texts each, so the report attributes them to Exact itself. Flattening each row from
  5 text nodes to 2 fixed the earlier hitch and touch-lag findings.
- Installed on simulator EEFC0029-… as `dev.bakeoff.notes.b884ecb` ("Notes"). Launched by
  hand to confirm it shows real dates.

## What was hard / looked up / guessed
- Time: the data module may not read the clock, so the view passes
  `time.epochAtZero + performanceNow()` into each save (the `dates-and-days` recipe).
- Seeding: I guessed that `import seed from './data/notes.json'` in `app.ts` is the right way
  to ship the data, and it worked. I store under a different file name in case `app:/data/` maps
  near the project's `data/`.
- Syntax hits: `.replace` → `replaceAll`, a computed route must be `path("note", id)`, a
  block `match` takes no `=>`, and a section `header` can't sit under an `if`. I wrote two
  sections instead. The test step to go back is a bare `back`, not `tap "back"`.
- The editor's bar has an empty heading, because Notes shows no title there. The pin button
  uses the same `symbol:pin` for both states: `sf/pin.slash` warns because it draws blank on
  web/Linux. The state shows in the list's sections and the menu's "Unpin Note".
- A long body scrolls inside its textarea, which has a minimum height, rather than the whole
  page scrolling with the title.

## Not done / could be better
- Pin is only a long-press menu item, not a leading swipe. The swipe long form (snap scroll
  with `swipeLeading`) looked fragile next to UIKit's native `delete=` swipe.
- No undo after a swipe delete, which matches Notes but has no Recently Deleted folder.

## What would have made it faster
- A `notes` pattern: a list with an autosaving editor and discard-if-empty on back. That is
  the most common app shape, and I had to work out the save-on-input plus leave-hook design.
- A clear note on what `navigationBack` covers. I assumed, without checking, that the iOS
  edge-swipe back gesture runs the same action, so the discard runs.
