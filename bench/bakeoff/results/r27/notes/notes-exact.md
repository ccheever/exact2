# Notes (Exact) — build notes

Built with Exact, starting from the `list-detail` pattern. The finished app (`dev.bakeoff.notes.bf63175`, "Notes")
is installed on simulator CEDFB9C9-FF04-4617-B156-94C56B83CA86. `bun exact.mjs verify` and
`bun exact.mjs verify ios` both report **clean**: 7 tests pass on web and iOS, no findings in light or dark,
nothing spills at the largest accessibility text size, and the scroll feel check has nothing to fix.

## What it does
- **List:** a large "Notes" title with the note count as the bar's subtitle ("90 Notes"), a compose button and
  the bar's search field. Pinned notes get their own "Pinned" section, then "Notes". Each section is sorted most
  recently updated first. A row shows the title, then a Mail/Notes-style date (the time if today, "Yesterday",
  the weekday, else 9/9/26) and the first line of the body. A note with no title is named by its body's first line.
- **Search** filters by title or body on every keystroke. When nothing matches, an empty state takes the list's place.
- **Editor:** tapping a row pushes the note, with a borderless title field and body. Every edit saves right away
  (there is no Save button) and stamps the note's `updated` time, so it moves to the top. The bar has Pin/Unpin
  and Delete; Delete asks before deleting.
- **New note:** the compose button opens an empty editor with the title focused. The note is only written once it
  has words in it. A note left empty (never typed in, or typed in and then cleared) disappears from the list, and
  is removed from the file at the next launch.
- **Pin/unpin and delete from the list:** a long press opens a menu with Pin Note/Unpin Note and Delete. Swiping
  a row deletes it.
- **Persistence:** on first launch the notes are seeded from `data/notes.json`, which `app.ts` imports, so it is
  bundled into the app. After that everything lives in `app:/data/notes.json`, and the tests check it across
  relaunches.

## What was hard / what I guessed
- **Where the editor's state lives.** The rows are `link`s, so a tap goes through `follow(url)`. The drafts sit
  in the root and `follow` fills them from the note. A new note gets its id from the clock (`n<ms>`), so the
  first keystroke can create it and later ones update it, in order, through a `queue` mutation.
- **Discarding an empty note.** Exact has no "on leave" hook that would let the root see the drafts. So the data
  module never creates a note with no words, and `loadNotes` leaves empty notes out. Clearing an existing note
  keeps its pin in the file until the next launch.
- **The editor's Delete button.** With `commandfor`/`command="show-modal"` the button did not appear in the iOS
  bar. Using `press=` with `showModal(...)` fixed it.
- **The unpin icon.** `sf/pin.slash` set off warnings, because the project also ships web and Linux. I added an
  `app.tamber` icon token with a hand-drawn SVG fallback.
- **The largest text size.** At the largest text size, verify flagged the date and preview line, which shared one
  line. That line now wraps, so the preview drops under the date at huge sizes; it is still one line.
- **Swipe to pin.** Swipe-right-to-pin would need Exact's "long form" swipe (a horizontally snapping scroll), so
  pinning from the list is done through the long-press menu instead.

## Not done
- Swipe-right-to-pin (pinning works from the long-press menu and the editor).
- No "Recently Deleted"; deleting is permanent. The editor's Delete asks first; a swipe deletes at once, as iOS does.
- Return in the title field doesn't move focus to the body; I couldn't find a focus API in the time I spent looking.

## What would have made it faster
- A short recipe for an "autosaving document editor": a pushed screen whose fields save on every edit, plus a
  way to run an action when a route is popped (for "discard if empty").
- Leading and trailing swipe actions next to `delete=` with no long form (for example `swipeLeading=` taking a button).
