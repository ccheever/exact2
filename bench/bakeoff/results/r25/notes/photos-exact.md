# Photos (Exact) — build notes

## What was built

- **Library tab**: a virtualized grid, four across with 2-pt gaps, of square centre-cropped thumbnails
  (`object-fit="cover"`). The oldest photo is at the top and the grid opens at the newest
  (`scroll-start="end"`). A day heading comes before each day's photos: Today, Yesterday, a weekday
  for the last week, `Sat, Oct 18` this year, `Oct 18, 2025` before that. Days follow the device's
  own time zone (`exactTime().utcOffset`). Two photos taken in the same minute are ordered by id.
- **Viewer**: a full-screen route (`navigationPresentation="fullscreen"`) that opens with iOS's
  zoom transition from the thumbnail (`navigationSource`). Swiping down zooms it back into the
  grid; that is UIKit's own interactive dismissal. The photos are pages in a horizontal virtualized
  list with x-mandatory scroll snapping, each drawn at its own aspect ratio (`object-fit="contain"`).
  The photo showing comes from the list's `ScrollEvent.firstVisibleKey`. Once the pager stops, the
  grid behind it scrolls to that photo, so closing lands on the right thumbnail. Tapping the photo
  hides the bars and shows it on black. The bottom bar has Favorite and Delete; Delete asks first,
  then the next photo shows, or the viewer closes if none are left.
- **Favorites**: the heart in the viewer, plus a white heart at the bottom left of favorite
  thumbnails. A long press on a thumbnail opens a Favorite/Delete menu.
- **Albums tab**: the eight albums in an inset-grouped list, each with its newest photo as the
  cover and its count, plus a Favorites album under "Utilities". An album opens its own grid (no day
  headings, as in Photos), with the same viewer and select mode. An album with no photos left shows
  an empty state.
- **Select mode**: Select → tap photos (blue checkmark and a light veil) → the bar shows "N Photos
  Selected" with Favorite (or Unfavorite when every selected photo is already a favorite), Delete
  (asks first) and Cancel.
- **Persistence**: `app.ts` writes `app:/data/library.json` with the favorites and the deleted ids.
  The shipped `favorite` flags are the starting set. Tests cover relaunching after favoriting,
  bulk favoriting and deleting.
- **Shipping the data**: `data/photos.json` is imported by `app.ts` and also copied to
  `assets/photos.json`. The full images are in `assets/images/`. 360-px thumbnails, made with
  `sips`, are in `assets/thumbs/` so the grid doesn't decode 1,200-px JPEGs.
- An app icon (a drawn petal flower) is in `assets/icon-1024.png`. The script that draws it is
  `.exact/tools/icon.py`.

## What was hard / what I had to look up

- **A horizontal virtualized list needs a literal numeric `height`.** A full-screen pager can't
  know its height ahead of time. I got around it with `height=2000 max-height="100%"` inside a
  flex child. It works on iOS and the web, but it is a workaround, not a documented pattern.
- **Opening the pager at the tapped photo.** `scrollLeft = index * width` landed on the wrong page,
  because the virtualized list's estimated widths differ from the real ones. Calling
  `scrollIntoView("pager", id, inline="start")` in the action that pushes the route works.
  Working out the current page from `firstVisibleKey`/`firstVisibleOffset` is also more reliable
  than dividing the offset by the width.
- **Swipe down to close** comes free with a fullscreen route that names a `navigationSource`
  (UIKit's zoom transition). I only found that by reading `host/apple/.../ModalIOS.swift`. A
  "photo viewer" recipe would have saved that search.
- `scrollIntoView` needs a literal id, so the two grids are `grid-all` and `grid-album`, not an
  interpolated id. A `derive` can't be a lambda.
- The `navigationScroll` route attribute has to name the grid's id, or tapping the status bar
  won't scroll to the top (found by `verify ios`).
- Verify's relaunch check picks the test whose resources held the most. My albums test only read
  data, so it reported "lost". I made that test favorite a photo so the relaunch means something.

## Guesses

- Photos-style day headings, album order (alphabetical), the album cover (the newest photo), and
  the "Favorites" utility album are my choices; the spec doesn't set them.
- The delete confirmations are iOS alerts. Photos uses an action sheet; with `closedby="any"`,
  Exact chose an alert here.
- Deleting removes the photos for good (there is no "Recently Deleted" album).

## Not finished / known limits

- No pinch-to-zoom in the viewer, and the status bar doesn't hide with the viewer's bars.
- No drag-to-select across thumbnails in select mode; you tap each one.
- On the **web** build only, reloading the page at a non-root URL (`/albums`) fails to boot in
  verify's headless browser. iOS always launches at `/`, so the iPhone app isn't affected. I didn't
  get to the bottom of it.
- The compiler warns about the literal white and black colours on the photo overlays (the heart,
  the checkmark, the black viewer background). They are deliberate: they sit on photos, not on
  themed surfaces.

## What would have made this faster

- A `pattern photo-grid` or a recipe for a virtualized grid → full-screen pager → zoom-back flow.
  It is a very common app shape, and every piece exists in Exact, but finding them meant reading
  `apps/shared-elements`, `apps/carousel` and the Swift host.
- Letting a horizontal virtualized list fill its parent (`height="100%"`/`flex=1`).
- A documented way to have the agent screenshot capture the context-menu platter.
