# Photos (Exact) — build notes

Built with Exact from the `tabs` pattern. Installed on simulator `498D6D1F-…` as
`dev.bakeoff.photos.b7891c0` ("Photos"). `bun exact.mjs verify ios`: 5 tests pass. Its checks
(light, dark, largest text, relaunch) find nothing, and scrolling is "like UIKit". The only
findings left are 7 compiler warnings about literal colours (see below).

## What's where
- `app.ts` imports `data/photos.json`, sorts it oldest first and answers four sources:
  `photos(album)`, `grid(album, utcOffset, now)` (day headings plus rows of four),
  `albums()`, and the mutations `setFavorite(ids, fav)` and `deletePhotos(ids)`.
  Favourites and deletions go to `app:/data/edits.json`, so they survive a relaunch.
- `assets/photos/` holds the 240 shipped JPEGs, unchanged. `assets/thumbs/` holds
  300 px square centre crops of them, made with `sips`, so the grid doesn't decode
  1200 px images.
- `app.contract` has these parts:
  - **Library grid.** A virtualized list opened at the end (`scroll-start="end"`), with
    four square cells across and 2 px gaps.
  - **Albums tab.** A two-column grid of covers with counts, each pushing its album's grid.
  - **Select mode.** The tab bar is hidden (`display="none"`) and a toolbar takes its place,
    with Favorite/Unfavorite all, a count, and Delete behind a confirmation alert.
  - **Viewer.** A full-screen overlay drawn after the tab panels. A `pan`/`panrelease` drag
    locks to one axis. Horizontally, the neighbouring pages slide with a transition.
    Vertically, the photo shrinks and the background fades to show the grid, and letting go
    past 120 pt (or a quick flick) closes it, scrolling the grid to the photo shown. A tap
    hides or shows the bars. The bars have Back, the date and time, Favorite, Delete, and
    Previous/Next buttons, which also give VoiceOver a way to page.

## What was hard / what I had to guess
- **Choosing the viewer's shape.** Exact has no pager or gesture component. I based the
  viewer on the `spark` sample's pan-and-transition deck. I made it an overlay rather than a
  route so the grid shows through while you drag it away. The cost: web `verify` at the
  largest text size reports "overlap" between the viewer and the grid under it. The overlay
  covers the grid on purpose, and the iOS large-text check passes.
- **Each grid row needs one root.** A `for` in a virtualized list must have a single element
  root, so a heading-or-row choice has to be wrapped in a `column`.
- **`estimated-item-height` must be a literal**, even though the cell size comes from the
  viewport.
- **Test ids are shared across tabs.** Both tabs' stacks stay mounted, so album cells are
  prefixed with the album name (`Pets-photo-6`).
- **Literal colours.** The heart, selection ring and checkmark are white and the hidden-bars
  viewer background is black, the same in light and dark mode as in Photos. Exact warns about
  these literal colours (`lower-colour-one-scheme`), and I left them.
- **Day headings.** "Today", "Yesterday", the weekday within the week, else "Sat, Oct 24"
  (with the year when it's not this year). This is my guess at Photos' style. The date
  arithmetic is in the data module, using the zone's current UTC offset.
- **Album cover** is the album's newest photo.
- **Delete** asks first, with an alert, and has no Recently Deleted album.

## Not done
- No pinch or double-tap zoom in the viewer, no drag-to-select, no zoom transition from
  the thumbnail into the viewer (the viewer fades and scales in instead), and no Favorites
  smart album. None of these is in the spec.

## What would have made this faster
- A built-in paged horizontal viewer, or swipe-to-dismiss for a full-screen route, with a
  recipe for it.
- A "photo grid" pattern: a virtualized grid with section headings that opens at the end.
- A way to mark an intentional full-screen overlay so the large-text overlap check skips it.
