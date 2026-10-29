# Photos (Exact): notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` tests).
Installed on simulator 486FD498-0E3E-4B64-8E4B-152BD8265EE7 as `dev.bakeoff.photos.bb42d69` ("Photos").
`bun exact.mjs verify` (web) is clean, and the 7 tests pass on web and iOS. `verify ios` has one finding,
covered under "Not finished or uncertain" below.

## What's there
- **Library tab:** a virtualized grid of all 1,500 photos, four across with 2 pt gaps, square centre-crop thumbnails.
  Oldest photos are at the top, and the grid opens at the newest. Each day has a heading: Today, Yesterday,
  a weekday name for the last week, then "Friday, Oct 23". A "1,500 Photos" footer sits at the bottom, as in Photos.
- **Viewer:** full screen, each photo at its own aspect ratio (`object-fit: contain`). It opens with UIKit's
  zoom transition from the thumbnail, so swipe-down-to-close is UIKit's own interactive dismissal. Swiping left
  and right pages through the photos of the grid you opened it from (the library or an album). The bar shows the
  photo's day and time, a heart (favorite or unfavorite) and a trash button that asks before deleting.
  When you close the viewer, the grid scrolls to the photo you were last on.
- **Hearts:** favorites show a heart on their thumbnail.
- **Albums tab:** all eight albums in a two-column grid, each with its newest photo as the cover, its name and
  its count. At the largest text size it drops to one column. An album opens a continuous grid of its photos.
- **Select mode:** "Select" in the bar switches the grid to select mode. Tapping photos checks them, the tab bar
  becomes a toolbar with Favorite and Delete, and Delete confirms first. Favorite sets every selected photo as a
  favorite, or unfavorites them all if every one already was.
- **Persistence:** favorites and deletions are kept in three persisted id lists: favorited, unfavorited, and
  deleted. They are applied over the shipped `photos.json`, so its own `favorite` flags still count.
- **Shipped data:** the data and images ship in the app. `data/photos.json` is imported by `app.ts`. The full
  images are in `assets/photos/`, and 300 px centre-cropped thumbnails I generated with `sips` are in
  `assets/thumbs/`, so the grid doesn't decode 1,200 px JPEGs. Only 240 distinct image files exist; the 1,500
  records reuse them.

## What was hard
- **The pager.** A horizontal `list virtualized=true` needs a *literal* pixel height (`lower-collection-cross`).
  `100%`, `100vh` and a state value were all refused, and the screen height isn't known at compile time. So I
  wrote my own windowing: a snap-scrolling `scroll` that builds only the 5 pages around the current photo, with
  spacer boxes sized `index × pageWidth` on each side. The page width comes from `frame()` of the grid when a
  photo is opened. The current photo is worked out from `scrollLeft / clientWidth` in the `scroll` handler.
- **The navigation bar on the full-screen route.** The `header` only becomes the iOS nav bar when it is a direct
  child of the route's column and comes first. Inside a component's `match`, or after the pager, it rendered as
  plain content. On the web, though, the header has to come before the absolutely-positioned pager to be
  tappable. The fix was header first, with the pager in normal flow (`flex=1`).
- **Hidden header buttons.** A header button that opens a dialog with `commandfor` is hidden on iOS
  (`lower-header-unplaced`). It has to have a `press` action that calls `showModal()`.
- **A dark viewer.** `color-scheme="dark"` on the viewer route made the bar's title black on black. I used the
  system background instead, which is what Photos shows while its chrome is visible.

## Looked up or guessed
- I found the zoom presentation (`navigationPresentation="fullscreen"` plus `navigationSource`) in
  `apps/shared-elements`. I guessed, then confirmed with a real-touch drag, that it gives swipe-down dismissal.
- Day headings use the device's time zone (from `Date` in `app.ts`). Under the agent's virtual clock, "today"
  is 2026-01-01, so test screenshots show full dates; a real launch shows "Yesterday" and the weekday names.
- Albums are listed alphabetically, since the spec gives no order.
- Deleting is permanent (there is no "Recently Deleted" album), so it always asks for confirmation.

## Not finished or uncertain
- `verify ios` reports one scrolling finding, "tracking ratio 0.001", on the first screen's list, and says it is
  Exact's own when there is no `scroll=` handler (the grid has none). Scrolling the grid by hand with real touches
  works normally. I believe the probe drags at the end of a list that opens at its end, but I couldn't confirm it.
- There is no pinch-to-zoom in the viewer, no tap to hide the chrome, and no share or info buttons (none were
  in the spec).

## What would have made this faster
- A documented horizontal pager or carousel recipe: a full-screen, snap-paging, virtualized strip whose height
  is its container's.
- Saying up front that a route's `header` must be its first direct child to become the nav bar.
- A list-equality form for `expect state` (`== [1, 2]`), which currently only takes scalars or `[]`.
