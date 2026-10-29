# Notes (Exact): build notes

## What it does
- **List:** pinned notes come first in a "Pinned" section, then the rest under "Notes", each sorted by most recent update. A row shows the title, a Mail-style date (time today, "Yesterday", the weekday, else 3/7/26) and the first non-empty line of the body. A note with no title or body shows "New Note" / "No additional text".
- **Count:** the large title's subtitle shows the count ("90 Notes", or "1 Note").
- **Search:** the bar's search field filters by title or body as you type. When nothing matches, an empty state says so.
- **Editor:** tapping a note pushes an editor with a borderless title (wraps, grows) and body. Every keystroke saves; there is no Save button. The note gets a new `updated` time, so it moves to the top. Return in the title moves to the body. The bar has Pin/Unpin and Delete (Delete asks first).
- **New note:** the compose button opens an empty editor with the title focused. Nothing is written until the note has text. A note left empty (new, or emptied) is deleted when you leave it, whether by the Back button or the swipe back.
- **From the list:** swipe a row to delete it; long-press it for a menu with Pin Note/Unpin Note and Delete.
- **Saving:** notes are kept in `app:/data/notes.json`. The first launch seeds it from `data/notes.json`, which `app.ts` imports, so it is bundled.
- **Tests:** 7 tests in `app.test.contract`. `verify` (web) passes all of them. `verify ios` is clean: light and dark, the largest text size, scrolling, and relaunch.

## What was hard or guessed
- **Back doesn't always call the back action.** The test step `back` (and the browser's Back) reaches the app through `navigate=` (`follow`), not `navigationBack`. My "discard empty note" logic at first lived only in `back`, and a test caught it. It now runs in both.
- **A block `match` doesn't take `=>`.** The compiler said so clearly.
- **A section header that comes and goes** has to be two `section`s under `if`/`else`.
- **The title field:** a single-line `input` clipped long titles at large text sizes. I switched to a `textarea rows=1 field-sizing="content"` that strips newlines and moves focus to the body (my guess at Notes' behaviour).
- **Pin icon:** `symbol:sf/pin.slash` warns because the app also ships web and Linux. I kept a single `symbol:pin` whose accessibility label switches between Pin and Unpin.
- **Search finding:** I guessed that "search" should also match the body (the spec says title or body).
- **Emptied notes:** I guessed that an existing note emptied in the editor is discarded too, as Apple Notes does.

## Not finished / known nits
- **One web-only `verify` finding:** at the largest text size there is a horizontal overflow on the frame right after tapping a row, mid push-transition. Checking the settled screen at the same size finds nothing, and `verify ios` is clean. I believe it is a transient artifact of the transition.
- **Pin from the list** is in the long-press menu only, not a leading swipe action. Mixing the long-form swipe with `delete=` looked riskier than it was worth.

## What would have made this faster
- A list-detail pattern variant whose detail is an editor that saves as you type, with "discard if empty on leave".
- Docs saying plainly that a route pop can arrive through `navigate=` as well as `navigationBack`.
