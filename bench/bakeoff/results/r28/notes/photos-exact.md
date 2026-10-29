# Photos (Exact): notes

Built with Exact and installed on simulator `CEDFB9C9-…` as `dev.bakeoff.photos.b288e71`, named "Photos".
`bun exact.mjs verify` (web) passes all 5 tests, and `bun exact.mjs verify ios` is **clean**: tests pass, no
light/dark findings, nothing spills at the largest text size, and scrolling is "like UIKit".

## What's there

- **Library grid** (`PhotoGrid`): a virtualized list of rows that `app.ts` builds. Each row has four square,
  centre-cropped thumbnails (`object-fit: cover`) with 2 pt gaps. The oldest photo is at the top, and the grid opens at the
  newest (`scroll-start="end"`). Each day gets a heading in the person's time zone, and the last two days read
  "Today" and "Yesterday". A "1,500 Photos" footer ends the grid. A heart marks favorites.
- **Viewer**: a full-screen route over the tab bar. It shows the photo at its own aspect ratio, black in both
  schemes, with the date and time at the top and Favorite, the album name and Delete at the bottom.
  - Swipe left or right to turn the page; the neighbouring photo slides in.
  - Drag down to close; the backdrop fades as you drag.
  - Pinch to zoom up to 4×; double-tap toggles 2×.
  - The route is the photo's (`/photo/3`, `/albums/Travel/photo/3`), so a reload reopens it.
  - Closing scrolls the grid to the photo you were on.
- **Albums tab**: the eight albums, two to a row, each with a cover (its newest photo) and a count. Cards drop
  to one column at large text sizes. Tapping one opens a four-across grid of its photos.
- **Select mode** works in the Library and in any album. Select / Cancel is in the nav bar, and tapping a
  thumbnail marks it with a check. While selecting, a bottom bar replaces the tab bar with Favorite (or Unfavorite when every
  selected photo is already a favorite), a count, and Delete, which asks first.
- **Persistence**: favorites and deletions go to `app:/data/edits.json`, which is read over the bundled
  `data/photos.json` and never writes to it. Tests confirm both survive a relaunch.
- **Shipped data**: `app.ts` imports `data/photos.json`, so it is bundled. The 240 JPEGs are bundled as
  `assets/photos/` (originals) and `assets/thumbs/` (300 px on the short side, made with `sips`, so the grid
  doesn't decode 1,200 px images).

## What was hard

- **Paging plus drag-to-close.** Exact has no pager and gives one gesture owner per target
  (`transformDragFor`). I adapted Excord's image viewer: the photo and its two neighbours sit in one transformed
  box, and the release decides what happened (down closes, sideways turns, otherwise spring back or keep the
  zoom). A page turn animates the strip one page over. Then, after 260 ms, the new photo becomes current and the
  strip jumps back to 0 with the transition switched off for one frame, so there is no visible jump (I filmed
  it on the simulator to check).
- **The drag is damped at fit.** At fit the host rubber-bands the drag, so the release offset is about half the
  finger's travel. A 120 pt swipe didn't turn the page until I lowered the threshold to ⅛ of the width (or a
  400 pt/s fling).
- **Compiler rules I hit one at a time:**
  - A virtualized list takes exactly one `for` with one root element, so the footer became a row from `app.ts`.
  - `scrollIntoView` needs a literal list id.
  - No `xs[i]`.
  - `press=` can't take a conditional.
  All were clearly worded and quick to fix.

## Looked up or guessed

- I found the viewer pattern by grepping the pinned checkout's `apps/` (Excord's `viewer.contract`,
  interaction-gallery) and `docs/agent-pitfalls.md`. It isn't among the `pattern` starters.
- The colour checker flags literal colours that are deliberately the same in both schemes: the viewer's black, and
  the white heart and check over photos. I wrote them as `light-dark(x, x)`, which states that intent and clears
  the warning.
- I guessed that Photos-like day headings are fine without the year, which is added only when a day falls in a different
  year from the newest photo.
- Album order is the fixed order of the eight names; the spec gives none.

## Mistake

Early on, a stray `sips -Z` resized the JPEGs in `data/images/` in place. I restored them from the copy already
made in `assets/photos/` and checked every file's dimensions against `data/images/images.json`.

## Not done / limits

- At the largest text size on the **web** build, the select bar (an overlay) and the header's "Select" text
  are flagged. The iOS build, the target, reports nothing there.
- Select mode has no drag-to-select-many (Photos' swipe across thumbnails). It is tap to toggle.
- The viewer has no info panel or share button; the spec doesn't ask for them.

## What would have made this faster

- A `pattern` (or recipe) for a photo grid plus pager viewer. Combining a horizontal pager with drag-down-to-close
  was most of the design time.
- A note in the drag docs that the release offset is damped at fit.
- A way to say "this colour is intentionally scheme-independent" that the docs name, rather than a
  `light-dark(x, x)` trick.
