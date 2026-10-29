# Photos (Exact): notes

The app is built and installed on simulator `C2188CC6-…` as `dev.bakeoff.photos.b683112` ("Photos").
`bun exact.mjs test web` passes 7/7, and so does `verify ios`. Check (light, dark) and the
largest text size report no findings on iOS.

## What's in it
- **Library tab:** a virtualized grid of all 1,500 photos, four across with 2 pt gaps and square
  centre-cropped thumbnails. Oldest is at the top. It opens scrolled to the newest at the bottom
  (`scroll-start="end"`). Each day has a heading (e.g. "Sunday, October 26"), and the foot shows
  "1,500 Photos, 80 Favorites". Favorites have a white heart in the bottom-left corner.
- **Viewer:** a full-screen route (`navigationPresentation="fullscreen"`, zooming out of its
  thumbnail on iOS 18+) on black. Each photo shows at its own aspect ratio (`object-fit: contain`).
  - Swipe sideways to page. It's a scroll-snap strip that builds only the pages near the current
    one, with spacers so every page keeps its offset.
  - Drag down to close (the transform drag from Excord's viewer; the backdrop fades as you drag).
  - Double-tap zooms, and pinch works too.
  - Buttons for Close, Favorite/Unfavorite and Delete (which asks first), plus the date, time,
    album and "n of N". On close, the grid scrolls to the photo you were last on.
- **Albums tab:** the 8 albums in a two-column grid, each with its newest photo as cover and a
  count. At large text sizes it drops to one column. An album opens its own grid, which opens at
  the newest and has a count footer.
- **Select mode** (in the Library and in albums):
  - Select/Cancel in the nav bar; the title shows "N Photos Selected".
  - Tapping toggles a blue check.
  - A bottom toolbar replaces the tab bar. Its Favorite button unfavorites when every selected
    photo is already a favorite, and its Delete asks first.
- **Long-press menu** on a thumbnail: Favorite/Unfavorite and Delete.
- **Saving:**
  - `app.ts` writes `app:/data/library.json`. It holds only the favorites that differ from the
    shipped data, plus the deleted ids.
  - An overlay shows favorites and deletions as soon as you tap.
  - Album counts and covers update after deletions.
- **Shipped data:** `data/photos.json` is imported by `app.ts`. The images are in
  `assets/photos/` (copied from `data/images/`). `assets/thumbs/` holds 300 px square
  thumbnails I made with `sips`, so the grid never decodes 1,200 px JPEGs.

## What was hard / guessed
- **Paging viewer:**
  - A horizontal `list virtualized=true` needs a literal pixel `height`, which a full-screen
    pager can't give. I hand-windowed a plain `scroll` instead: spacer boxes plus five built pages.
  - `scroll-snap-stop` isn't supported, so a fast fling could in principle skip pages. In
    testing, single swipes moved one page.
  - The dismiss drag has `touch-action="pan-x"`, which hands horizontal moves to the pager. I got
    that from reading LLP 1057.001's precedence rules, not from a recipe.
- **Grouping by day:** this is done in `app.ts` in the device's local time zone. I guessed that
  this matches Photos' behaviour; the data is in UTC.
- **Relaunch:** on the web, reloading inside the viewer reopens it at the same photo. On iOS a
  relaunch returns to the grid; I didn't find whether that's by design.
- **`verify ios` scrolling report:** it flags "glide after a still lift 196 pt" on the grid. The
  grid has no `scroll=` handler and no snapping, and the report puts this down to Exact itself
  ("not yours to fix"), along with small hitch "tells". I couldn't fix it from the app; changing
  `estimated-item-height` didn't help.
- **Fixed colours:** the remaining compiler warnings are the deliberate white overlays on photos
  (heart, selection ring) and the viewer's black backdrop.

## Not done
- Drag-across-to-select in select mode, sharing, and Recently Deleted (deletions are final after
  the confirmation).

## What would have made it faster
- A pattern or recipe for a photo grid plus a paging viewer: the gallery shape is common and I
  had to piece it together from `shared-elements`, Excord's `viewer.contract` and the gesture
  LLP.
- The `lower-collection-cross` error could suggest a fix for a full-screen horizontal pager.
- `contract vocab` returned nothing for "toolbar" or "swipedown". Pointing those words to
  `bar-buttons` and the transform-drag dismiss would have saved some searching.
