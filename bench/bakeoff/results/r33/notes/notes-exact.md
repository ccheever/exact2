# Notes: build notes

Built with Exact from the `list-detail` pattern. `app.contract` is the view, `app.ts` the data module
(a JSON file at `app:/data/notes.json`, seeded from `data/notes.json` the first time the app runs),
and `app.test.contract` has 6 tests. `verify` (web) and `verify ios` both finish clean: the tests
pass in light and dark, nothing spills at the largest text size, and scrolling has nothing to fix.
The app is installed on simulator 498D6D1F… as `dev.bakeoff.notes.bba1d79` ("Notes").

## How the spec maps to the app
1. The list has a **Pinned** section, then a **Notes** section (that header shows only when something
   is pinned). Each is sorted by `updated`, newest first. A row shows the title, then a Mail-style date
   (time today, Yesterday, weekday, else 9/27/26) and the first line of the body that has text.
2. The search field in the bar filters by title or body as you type, ignoring case. When nothing
   matches, an empty state says so.
3. Tapping a row pushes an editor with the date, a bold title field and the body, both borderless as in
   Notes. Every keystroke sends a queued `saveNote` with the current time, so the note moves to the top.
   There is no Save button.
4. The compose button in the bar opens a draft with a fresh id. The note is first written to the file
   on the first keystroke. If you leave it empty (including clearing a note to nothing), Back deletes
   it. Empty notes are never listed, and any left in the file are dropped on the next launch.
5. Long-press a row to Pin, Unpin or Delete it. Swiping a row deletes it (UIKit's swipe action). The
   editor's bar also has Pin and Delete; Delete there asks first.
6. The count ("90 Notes") is the large title's subtitle in the navigation bar.
7. Everything is kept in a file and survives relaunch (tested with `reload`, and checked by verify's
   relaunch).

## What was hard / what I had to look up or guess
- **The one-line preview at the largest text size.** `line-clamp=1` is flagged as clipping at 53 pt.
  At that size a one-line clamp shows about 10 characters. The fix: `app.ts` cuts the body's first line
  at a word near 28 characters (Contract has no `lastIndexOf`, so this moved into the data module as a
  `preview` field). It stays one line beside the date at normal sizes and wraps at the largest. The
  preview has `flex-basis="60%"`, so it drops under the date when the row is narrow. A first try with
  `min-width="8em"` overflowed, because em grows with the text size.
- **Clock.** The data module can't read the clock, so the view passes
  `time.epochAtZero + performanceNow()` with each save. Tests have to pin `epoch`: the default test
  epoch (2026-01-01) is older than the seed data, so a "new" note sorted below them.
- **Editor drafts.** Drafts live in root state and are filled in `follow` (a row is a `link`, so its
  navigation arrives there). This is because children can't declare mutations.
- **Guesses:** clearing an existing note completely deletes it on leave, as Apple Notes does. A note with
  an empty title shows "New Note" in the list. The un-pinned symbol is the same pin glyph (with the
  label "Unpin Note"), because `sf/pin.slash` draws blank on web and Linux.
- `contract verify`'s semantics pass reports "OUTSIDE (unsupported `toLowerCase`)" and a DIVERGE at
  boot for one script. These come from the toolkit's own oracle, not the app: every test passes on web
  and iOS.

## Not done
- A leading swipe-to-pin (Apple Notes style). The spec is met by the long-press menu, and the "long
  form" swipe of a hand-built scroll looked risky to combine with a `link` row and `delete=`.
- Return in the title doesn't move focus to the body (I found no focus-moving API quickly).

## What would have made this faster
- A built-in way to ellipsize text to one line that verify accepts at large sizes (as UIKit does with
  `numberOfLines = 1` plus Dynamic Type), or clearer guidance on what a one-line preview should do there.
- `lastIndexOf` in Contract, and `expect state` taking a simple filter.
- A pattern that already has an autosaving editor with an "empty draft is discarded" rule. That is a
  common app shape.
