# Notes (Exact): build notes

Installed on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7` as `dev.bakeoff.notes.b5aaac1` ("Notes").
`bun exact.mjs verify` (web) and `bun exact.mjs verify ios` both come back clean: 5 tests pass,
light and dark have no findings, nothing spills at the largest text size, scrolling has nothing
to fix, and the data survives a relaunch.

## What's there
- **The list.** Large title "Notes" with the count ("90 Notes") as the bar's subtitle, a compose
  button, and the bar's search field. A "Pinned" section comes first, then "Notes"; both are
  sorted newest first. Each row shows the title, then Mail/Notes-style date (`formatDay … "list"`:
  the time today, Yesterday, the weekday, else `10/26/26`) and the body's first non-empty line,
  each cut off to one line. If there are no notes, no search matches, or the read fails, an
  empty state takes the list's place.
- **Editing.** A tap pushes the note: its date, then the title and body as borderless,
  auto-growing text areas. Every keystroke saves through one queued mutation. A note only moves
  to the top when its text actually changes. Return in the title moves to the body. While
  editing, the bar shows Done (it puts the keyboard away) and Pin/Unpin.
- **New notes** are created at once and open with the title focused. Leaving one empty, by Back
  or the edge swipe, discards it. Notes left empty because the app quit are also dropped on the
  next load.
- **Pin, unpin and delete** from the list: swipe to delete, and a long-press menu with Pin/Unpin
  Note and Delete. The note screen also has its own pin button.
- **Persistence.** `app:/data/notes.json` holds the notes. On first launch, or if the file is
  missing, they are seeded from the shipped `data/notes.json`, which `app.ts` imports so it is
  bundled. ISO dates are converted to epoch milliseconds.

## What was hard / what I guessed
- **The time.** The data module can't read the clock, so every write takes the instant as an
  argument (`time.epochAtZero + performanceNow()`). The agent's clock defaults to
  2026-01-01, so edits made in ad-hoc drives looked "old" and sank down the list. That fooled me
  until I pinned `epoch` in the tests. A real launch uses the real time.
- **The title field.** A one-line `input` scrolled a long title sideways. I switched to a
  `textarea rows=1 field-sizing="content"` and remove newlines myself. I found
  `field-sizing` by grepping the long guide; the card doesn't cover it.
- **Tests.** Leaving a pushed screen in a test is the `back` step, not `tap "back"`. `verify`'s
  second engine reports "PART/DIVERGE/OUTSIDE" lines (`toLowerCase` unsupported, back/tap not
  delivered by the oracle). I took them as informational, since the summary said clean.
- **Symbols.** `sf/pin.slash` is blank on the web, so I added an `app.tamber` icon token with a
  hand-drawn SVG (`assets/icons/pin-slash.svg`). I dropped Linux with `update --without linux`.
- **Swipe-to-pin.** I left out a leading swipe action for pin. Exact's long-form swipe doesn't
  seem to combine with the platform-drawn `delete=` on a `link` row, so pinning lives in the
  long-press menu and on the note screen.

## Not finished / could be better
- The count is always the total, not the number of search matches.
- There's no share or move-to-folder, and a note screen has no delete button (delete is in the
  list, as the spec asks).
- Context-menu screenshots from `agent ios` are painted by the agent, so I checked the menu's
  behaviour in tests rather than how the real UIMenu looks.

## What would have made it faster
- A `notes` or `document-editor` pattern: a list that pushes an autosaving editor, with
  create-on-open and discard-if-empty.
- The card mentioning `field-sizing="content"` for text areas, and `back` as the test step that
  pops a screen.
- A default agent epoch near the real date, or a warning when a drive writes timestamps with the
  default clock.
