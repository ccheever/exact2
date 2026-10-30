# Photos (Exact) — build notes

Built with Exact (`app.contract`, `app.ts`, `app.test.contract`), installed on simulator
`EEFC0029-…6C96` as `dev.bakeoff.photos.b616146` / "Photos".

## What it does
- **Library tab**: a virtualized list. Each row is either a day heading ("Monday, October 26";
  the year is added for days outside the newest photo's year) or four square, centre-cropped
  thumbnails with 2-pt gaps. Oldest is at the top, and the list opens at the end
  (`scroll-start="end"`). The subtitle shows the photo count.
- **Viewer**: a `navigationPresentation="fullscreen"` route on black. The photo is
  `object-fit="contain"` at its own aspect ratio, and a low-res thumbnail sits under the full
  image while it loads. One drag handle (`transformDragFor` + `transformrelease`) follows the
  finger:
  - Released mostly sideways past 80 pt (or with a fast fling), it goes to the next or
    previous photo.
  - Released mostly downward past 120 pt, it closes.
  - Otherwise it springs back.

  A tap hides or shows the bars. The bars hold Close, the day, the time with "n of N",
  Favorite/Unfavorite, the album name, and Delete (which asks first).
- **Favorites**: a heart in the viewer. A small white heart marks favourites in the grid.
- **Albums tab**: the eight albums in a two-column grid of covers (the newest photo) with
  counts. Each opens its own grid, also oldest first and opened at the newest. Below them,
  under "Utilities", is a Favorites collection, kept separate from the eight albums.
- **Select mode** (in both the library and album grids):
  - "Select" starts it. Each tap toggles a checkmark, with a selection haptic.
  - The tab bar is hidden (`display="none"`) and a toolbar takes its place: Favorite (or
    Unfavorite, when everything selected is already a favourite), "N Photos Selected", and
    Delete.
  - Delete confirms with an alert.
- **Persistence**: `app.ts` writes the person's changes to `app:/data/library.json`:
  favourite overrides by id, plus the deleted ids. They are merged over the bundled catalogue
  on every load. Tests cover reloading after favouriting and after deleting.
- **Data shipped with the app**:
  - `data/photos.json` is imported by `app.ts`.
  - The images are copied to `assets/photos/`.
  - I generated thumbnails into `assets/thumbs/` with `sips`: about 320 px on the short
    side, JPEG quality 70. That way the grid doesn't decode 1,200-px JPEGs for each 98-pt cell.

## Verification
- `bun exact.mjs test web`: 5 of 5 tests pass. They cover:
  - grid order and day headings
  - viewer swipes left, right and down (simulated drags)
  - favouriting from the viewer, kept after a relaunch
  - select mode's favourite and delete, kept after a relaunch
  - album counts
- `bun exact.mjs verify ios`:
  - The tests pass on the simulator.
  - No findings in light or dark.
  - Nothing spills at the largest accessibility text size.
- I also drove it by hand with real touches (`agent ios --touch platform`): the swipes page and
  close, select mode works, the delete alert appears, and Albums and dark mode look right.

## What was hard / guessed
- **The full-screen pager's gestures.** Exact has no page-view component. A horizontal snap
  `scroll` would have to fight a vertical pan for the swipe-down. I copied the
  pinch/drag/dismiss approach from `apps/excord/viewer.contract` and
  `apps/interaction-gallery` and extended its release handler to step left and right.
  - Trade-off: the next photo appears in place after the drag springs back. It does not slide
    in the way a `UIPageViewController` does.
  - There is no pinch-to-zoom (the spec didn't ask for it).
- **Smaller compile errors.** `e.id` on a route entry is a number, not a string. `.every()` is
  refused. A `for` in a virtualized list needs exactly one root element.
- **Guessed:**
  - The day-heading format.
  - Album order (alphabetical) and covers (each album's newest photo).
  - Select-mode favourite toggles to *unfavourite* when every selected photo is already a
    favourite, as Photos does.
  - Day boundaries use the device's time zone.
- **Literal colours** (white glyphs, black backdrop, translucent bars) are kept on purpose:
  they sit over photos, and they trigger `lower-colour-one-scheme` warnings.
  `symbol:sf/rectangle.stack.fill` (the Albums tab icon) is Apple-only, so it is blank on
  the web build.

## Not finished / known issues
- `verify ios` reports one scrolling finding: a 197-pt glide after a still lift on the grid.
  Exact attributes it to itself ("Exact's own: report it"); the app has no scroll handler or
  snapping.
- On the web, at the largest text size, `verify` still notes:
  - the painted header's "Select" is clipped (iOS draws a UIKit bar button there, and it's
    fine)
  - the select toolbar overlaps grid cells, which scroll under it, as they do under a bar
- No pinch-to-zoom, no sliding transition between photos, no "Select All", and no
  drag-to-select across thumbnails.

## What would have made this faster
- A built-in horizontal pager (a paging scroll whose pages can also be dragged down to
  dismiss), or a pattern for "photo viewer with paging".
- A grid/collection-view primitive for virtualized lists. Chunking into rows of four in the
  data module works, but a `columns=4` on a virtualized list would be more natural and would
  adapt to width.
