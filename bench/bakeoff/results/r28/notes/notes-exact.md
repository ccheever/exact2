# Notes (Exact): build notes

Built from the `list-detail` pattern. `bun exact.mjs verify` (web) and `verify ios` both came back
clean: 5 tests passed; light and dark checks, the largest text size and the relaunch check found
nothing; scrolling had nothing for the app to fix. The app is installed on simulator
C2188CC6-FBE9-409A-92A8-ABD89AC2C432 as `dev.bakeoff.notes.bc8be55` ("Notes").

## How the spec maps to the app
- **List:** pinned notes get their own "Pinned" section above "Notes". Each section is sorted by
  `updated`, newest first. A row shows the title (bold), a Mail-style date (`formatDay … "list"`:
  the time today, "Yesterday", a weekday, or 10/3/26) and the first line of the body that has
  text, cut off with an ellipsis.
- **Search:** the search field in the navigation bar filters by title or body, ignoring case,
  as you type. If nothing matches, an empty state takes the list's place.
- **Edit:** tapping a row opens the editor, which has a borderless title field and a body text
  area. Every keystroke sends a queued `saveNote` mutation with the current time. The note moves
  to the top and the change is saved to the file; there is no Save button.
- **New note:** the compose button opens an empty editor with a client-made id (`n<epoch ms>`).
  Nothing is written until the first keystroke, so an untouched new note is never stored. If a
  note's title and body are both emptied, `saveNote` deletes it, as Apple Notes does.
- **Pin and delete:** swiping a row deletes it (UIKit's own swipe). A long press opens a menu
  with Pin Note / Unpin Note and Delete. The editor's bar also has a pin button.
- **Count:** the navigation bar's subtitle shows "90 Notes", or "N Notes Found" while
  searching.
- **Persistence:** `app.ts` imports `data/notes.json` as the seed. All writes go to
  `app:/data/notes.json`, and tests check that changes survive a relaunch.

## What was hard / guessed
- **Time in the data layer:** `app.ts` can't read the clock, so the view sends the time with
  each save (`time.epochAtZero + performanceNow()`). I worked this out from the
  dates-and-days recipe.
- **Leaving an empty new note:** I first planned to delete it in the back action. Making
  `saveNote` skip or delete empty notes was simpler, and it doesn't depend on which way the
  user leaves (back button, swipe back, or a wide-window replace).
- **Row preview:** the preview first wrapped under the date with a hanging indent.
  `white-space=nowrap` + `text-overflow=ellipsis` fixed it, and the largest-text check still
  passed.
- **Pin icon:** there is no "pin.slash" icon that works on every platform, so the editor's pin
  button uses the same icon for pinned and unpinned notes. Only its accessibility label
  (Pin/Unpin) changes.
- **Editor bar:** the bar has an empty heading, like Notes, which shows no title. Without an
  edited time, a new note's date comes from when it was opened.
- **Test data:** the counts in the search tests ("landlord" → 19 notes, "camera settings" → 23)
  came from checking the data file.

## Not done
- There is no leading swipe to pin. The swipe docs say a custom (long-form) swipe can't be
  combined with the platform's `delete=` swipe, so pinning is in the long-press menu and the
  editor instead.
- There is no delete button inside the editor (the spec asks for delete from the list only).

## What would have made it faster
- A `notes`-style pattern (a list plus an autosaving editor) would have been nearly the whole app.
- A role symbol for "unpin".
- Saying in start-here that `app.ts` can `import x from './data/file.json'` directly. It worked,
  but I had to guess.
