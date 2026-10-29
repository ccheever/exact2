# Notes (Exact) — build notes

## What it is
- `app.contract`: the list screen and the note editor. The list shows a large "Notes" title with the count as its subtitle, the bar's search field, a "Pinned" section, then the other notes. Rows are most recently updated first and show the title, a Mail-style date (`formatDay … "list"`) and a one-line preview. A row pushes the editor. Swiping a row deletes it, and long-pressing a row opens a menu with Pin/Unpin Note and Delete.
- Editor: borderless title and body fields that save as you type, with no Save button. Each edit stamps the note with the current time, so it moves to the top. The bar has a Pin toggle and a Done button that only shows while typing. Return in the title moves to the body.
- New note (compose button): it isn't stored until something is typed. Leaving a note whose title and body are both empty deletes it. Any empty note found at launch (say, the app was killed mid-edit) is dropped too.
- `app.ts`: the notes are kept in `app:/data/notes.json`. The first launch fills it from the shipped `data/notes.json`, with ISO dates turned into epoch ms.
- `app.test.contract`: 6 tests. `verify` (web) and `verify ios` are both clean: light/dark, largest text size, relaunch and scrolling.

## What was hard
- **A lost write.** An edit followed quickly by a pin each read the same old in-memory list, so the second save undid the first. The fix was to run every write in `app.ts` through one promise chain (`serial`). The contract's `queue` mutation alone didn't prevent this.
- **The pin button on web.** On the web host, tapping Pin while a field had focus blurred the field first. That hid the Done button, the header reflowed, and the click landed on nothing. Native iOS bar buttons don't have the problem. The test taps Done first.
- **Long titles.** A single-line `input` cut long titles off. I switched the title to a `textarea rows=1 field-sizing="content"` and strip newlines so Return moves focus to the body.
- Smaller fixes: a block `match` takes no `=>`, a computed route must be `path(...)`, and a section `header` can't sit inside an `if`. The compiler named each of these with its fix.

## Looked up / guessed
- Read the `list-detail` pattern and the `swipe`, `list-rows`, `menu`, `dates-and-days`, `files-and-grants`, `keyboard-and-focus` and `symbols` recipes.
- I guessed that leaving a note that was cleared to empty should delete it, as Apple Notes does. The spec only says this for new notes.
- Pin/unpin is in the long-press menu rather than a leading swipe. The swipe long form would have replaced the native `delete=` swipe. The unpin icon is the same `pin` symbol because `sf/pin.slash` warns that it would be blank on web/Linux.
- The count shows all notes, not the search results.

## Not finished / caveats
- No leading swipe-to-pin, no undo after delete, no share.
- Agent drives freeze the clock at 2026-01-01, so screenshots taken in drives show that date. The installed app uses real time.

## What would have made it faster
- A pattern close to "notes with an autosaving editor": that's a common app shape.
- A note in the docs that `queue` doesn't serialize the data module's own async work, or a built-in write lock.
