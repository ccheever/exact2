# Notes (Exact): build notes

**Status:** finished. All 7 spec features work. `bun exact.mjs verify` (web) and `verify ios` both came back clean:
6/6 tests pass, light and dark, the largest text size, a relaunch, and real-touch scrolling. The app
(`dev.bakeoff.notes.b31a8ce`, "Notes") is installed on simulator EEFC0029-… via `bun exact.mjs ios`.

## How it works
- `app.ts` imports `data/notes.json` as the seed. On first launch (no store file yet) it seeds from
  it, and after that it reads and writes `app:/data/store.json`. It returns notes most recently updated first.
- List: a large title "Notes" with the note count as the bar's subtitle ("90 Notes"). The bar's
  search field filters by title or body as you type. A "Pinned" section comes first, then "Notes".
  Each row shows the title, then a Mail-style date (the time today, a weekday this week, otherwise 9/27/26)
  and the first body line, truncated to one line.
- Swipe deletes. A long press opens a menu with Pin Note / Unpin Note and Delete. The editor's bar
  also has a pin button.
- Editor: a borderless title and body. Each keystroke saves through a queued mutation that stamps
  `updated` with the real clock, so the note moves to the top. New notes come from the compose button.
  A note is created by its first keystroke, and leaving a note whose title and body are both empty
  deletes it. That covers a new note left blank and an existing note cleared out.

## What was hard, guessed or looked up
- Leaving a screen arrives through two paths. One is the `navigationBack` action (the bar's Back).
  The other is `navigate=` with the parent URL (the browser or history Back, which the test's `back` step uses).
  At first I discarded empty notes only in the first path, and a test caught it. Both now do it.
- The data module has no clock, so the editor passes `epochAtZero + performanceNow()` in to
  `saveNote`. Under the agent's frozen clock (2026-01-01) an edit dates *older* than the seed data,
  so the edited note doesn't jump to the top there. The tests pin `epoch`, and real launches use real time.
- New note ids are the creation time in ms (Contract has no random and the module has no clock).
- The subtitle slot in the header was the only clean place for the count, because Exact has no bottom
  toolbar where Apple's Notes puts it.
- A grouped list's section header can't come and go under an `if`, so the "Notes" section is written twice.
- `sf/pin.slash` warns because the app also ships on Linux and the web, so I use the plain pin symbol for both states.

## Not done / would improve
- No swipe-to-pin (a leading swipe action). Pinning is in the long-press menu and the editor, which
  meets "from the list". The custom-swipe form looked fiddly next to `delete=`.
- No date-grouped sections (Today, Previous 7 Days…) as in Apple's Notes, and no "Done" button to hide
  the keyboard. You dismiss it by dragging.

## What would have made it faster
- A `note-editor` pattern (autosave, discard if empty, with the back paths unified).
- One documented "screen was left" hook that fires however the pop happens.
