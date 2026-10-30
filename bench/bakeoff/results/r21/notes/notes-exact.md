# Notes (Exact) — build notes

Built from `pattern list-detail`. The app is installed on simulator EF198926-… as `dev.bakeoff.notes.d8efeb` ("Notes").
`verify ios` is clean: 7 tests, light and dark, the largest text size, and relaunch. All 7 web tests pass.

## What's there
- The list puts the pinned section ("Pinned") first, then "Notes", each most recently updated first. Each row has the title, Notes-style date
  (`formatDay … "list"`: the time today, "Yesterday", a weekday, else 9/9/26) and the body's first non-empty line, clamped to one line.
  The relative dates update every minute.
- The note count is the navigation bar's subtitle ("90 Notes").
- The bar's search field filters by title or body on every keystroke. When nothing matches, an empty state takes the list's place.
- Tapping a row pushes an editor with a borderless title field and body field. Every keystroke sends a queued `updateNote` with the current time.
  A real change bumps `updated`, so the note moves to the top. There is no Save button.
- The compose button creates a note right away, opens it and focuses its title. If the note is still empty (whitespace counts as empty) when you
  leave it (Back, the edge swipe, or opening another note), it is deleted.
- Swipe left on a row to delete it. Long-press a row for a menu with Pin/Unpin Note and Delete. The editor's bar also has Pin, and Delete, which asks first.
- Storage: on first launch `data/notes.json` is imported by `app.ts` and bundled. After the first write, everything lives in `app:/data/notes.json`.

## What was hard / guessed
- The data module can't read the clock, so the action passes each write's time as `time.epochAtZero + performanceNow()`.
- The empty-note rule had to live in three places: `back` (which also handles the swipe), `follow` (links, and the wide split view), and `afterCreate`.
- The largest text size clipped the row's date and preview when they sat side by side. Wrapping them (`flex-wrap` with a `flex-basis` in rem) fixed it.
  Verify discourages `line-clamp` in rows, but the spec asks for a one-line preview, so I kept it. Verify no longer flags it.
- One finding from web `verify` (only at the largest text size): an "overflow app [-121,0]" at the end of one test, after a relaunch. I couldn't
  reproduce it with `agent web` at root-font-size 53 (`check` was clean), and `verify ios` doesn't report it. I think it's a snapshot taken mid-transition.
- `agent ios` drives use a virtual clock (2026-01-01 by default), so a note created during a drive shows "Today at 12:00 AM". Real launches use real time.
- Guessed details: the menu's wording ("Pin Note"/"Unpin Note"), and that a whitespace-only note counts as empty.
- Small syntax snags the compiler caught: a block `match` takes no `=>`, `font-weight` takes a number, and a section `header` can't be inside an `if`.

## Not done
- Leading swipe-to-pin as in Apple Notes. The long-form swipe and `delete=` didn't look like they combine, so Pin is in the long-press menu instead.
- Return in the title doesn't move focus to the body.

## What would have made it faster
- A pattern for a note-style editor (a borderless title and body that save as you type).
- A short list of the valid `font` role names on the card.
- A way to tell a real layout overflow from a mid-transition snapshot in verify's largest-text pass.
