# Photos (Exact): notes

Built, tested and installed on simulator `C2188CC6-…` as `dev.bakeoff.photos.b8bb382` ("Photos").
`bun exact.mjs verify` (web) and `verify ios` pass: 7 tests, no check findings in light or dark mode, and
nothing spills at the largest text size. Two of `verify ios`'s scrolling-feel numbers are flagged; see below.

## What's there
- **Library tab.** A virtualized grid of all 1,500 photos, oldest first, opening at the newest
  (`scroll-start="end"`). There's a day heading over each local day, square centre-cropped thumbnails
  four across with 2 pt gaps, a white heart on favorites, and a "1,500 Photos" footer.
- **Viewer.** A `navigationPresentation="fullscreen"` route whose `navigationSource` is the
  thumbnail's id, so UIKit's zoom transition opens it and its interactive dismissal handles swipe down.
  The photos page sideways in a horizontal virtualized list with `scroll-snap-type="x mandatory"`, each
  shown at its own aspect ratio (`object-fit=contain`). The header shows the date and time, and the
  bottom bar has Favorite, a position counter ("1495 of 1,500") and Delete, which asks first. Tapping
  the photo hides the bars on black. As you page, the grid scrolls so the current photo's thumbnail
  stays mounted, and the zoom back lands on the photo you're on.
- **Albums tab.** The eight albums in a two-column grid, each with its newest photo as the cover, its
  name and its count. An album opens an unbroken 4-across grid (no day headings, as in Photos) with the
  same viewer, select mode and delete.
- **Select mode** (Library and albums). Tapping toggles a checkmark, and the title says "N Photos
  Selected". A bar replaces the tab bar with Favorite and Delete; Favorite unfavorites if every selected
  photo is already a favorite. Delete asks first.
- **Persistence.** Favorites and deletions go to `app:/data/library.json` and are laid over the shipped
  `data/photos.json` at each launch.
- **Data shipping.** `photos.json` is imported by `app.ts`. The full images are in `assets/images/`. I
  also made 300 px square thumbnails in `assets/thumbs/` with `sips`, so the grid doesn't decode
  1,200 px JPEGs for 100 pt cells.

## What was hard / what I had to find
- **The pager.** A horizontal virtualized list must have a *literal* `height` and a literal
  `estimated-item-width`, and its flow root can't be positioned. So the list is 1,400 pt tall inside a
  clipped frame that measures itself (`resize=`), and each page gets the measured height.
- **Opening at the tapped photo.** `scrollLeft = index × width` landed on the wrong photo whenever the
  estimated width differed from the real one (on the web it was off by about 60 photos). The fix is an
  `after(1, …)` task that calls `scrollIntoView("pager", key)` once the route mounts. The current index
  comes from the `ScrollEvent`'s `firstVisibleKey`/`firstVisibleOffset`, not from pixel arithmetic.
- **`scrollIntoView` needs a literal list id**, so the two grids are `grid-library` and `grid-album`.
- **Finding the zoom/fullscreen route.** It isn't in start-here or the card. I found it by reading
  `apps/shared-elements` and LLP 1035.001, which also confirmed that `navigationSource` is resolved
  when UIKit asks, so it can follow the current photo.
- **Bottom toolbars.** There's no native bottom toolbar. Select mode's bar and the viewer's bar are
  my own rows of `buttonStyle="glass"` buttons.
- **The last grid row sat under the floating iOS 26 tab bar** when the list opened at its end. The
  footer row (count plus padding) fixes it.
- **Colours over photos.** The white heart and checkmark backing needed `scheme=fixed` tokens in an
  `app.tamber` to clear `lower-colour-one-scheme`.

## Guesses
- Day headings are full dates ("Monday, October 26, 2026") in the device's time zone, with no
  "Today"/"Yesterday".
- Deletion is permanent: there's no Recently Deleted album.
- Albums are sorted alphabetically. There's no Favorites smart album, since the spec says eight albums.
- Linux was removed from the app (`update --without linux`). Web still builds and passes.

## Not finished / known issues
- `verify ios`'s scroll-feel probe reported "tracking ratio 0.102" and "edge bounce settle 0.02 s" on
  the grid. The grid has no `scroll=` handler, so the report attributes these to Exact's own list. The
  machine's load was around 300 at the time, so the reading may also be noise.
- No pinch-to-zoom in the viewer, no share or info buttons, and no drag-to-select.

## What would have made it faster
- A pager recipe: a full-screen, horizontally paging virtualized list that opens at a given key, with
  a note on the literal-height rule.
- start-here listing `navigationPresentation="fullscreen"` and `navigationSource` (zoom and swipe
  down to close) for media viewers.
- A photo-grid pattern: a virtualized grid with sections, a select mode and a bottom action bar.
