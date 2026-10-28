# Photos (Exact) — build notes

Bundle id `dev.bakeoff.photos.b6716fd`, name `Photos`. It's built and installed on simulator
`C2188CC6-FBE9-409A-92A8-ABD89AC2C432`.

## What's there

- **Library grid** (`PhotoGrid`). It's a virtualized list with 4 square, centre-cropped
  thumbnails per row and 2 pt gaps. Oldest photos are at the top, each day has a heading
  ("Monday, Oct 26, 2026"), and the grid opens at the newest photo (`scroll-start="end"`).
  Favorites show a white heart in the bottom-left corner.
- **Viewer.** A fullscreen route presented with `navigationSource`, so on iOS 18+ UIKit's
  zoom transition grows it out of the thumbnail. Swiping down closes it back into the
  thumbnail; this is UIKit's own interactive dismiss, and I tested it with real touches.
  The photo is drawn at its own aspect ratio (`object-fit: contain`). Swiping left and right
  pages through a horizontal snap scroller.
  - The title shows the date and the subtitle shows the time.
  - The bottom bar has Favorite/Unfavorite (heart, with a selection haptic), "N of M", and
    Delete (confirmed with an alert).
  - Tapping the photo hides the bars and puts the photo on black, like Photos.
  - As you page, the grid behind scrolls to the current photo, so closing lands on the
    right thumbnail.
- **Albums tab.** A two-column grid of the 8 albums, each with a cover (its newest photo)
  and a count. Each album opens its own grid, which has the same viewer and select mode.
  At the largest text sizes it drops to one column.
- **Select mode.** Select/Cancel is in the nav bar, and the subtitle says "N Photos
  Selected". A bar above the tab bar has:
  - Favorite: favorites all the selected photos, or unfavorites them if every one is
    already a favorite.
  - Delete: asks for confirmation first.
- **Persistence.** `app.ts` keeps an edits file (`app:/data/edits.json`) holding favorite
  overrides against the shipped flags and the deleted ids. Album counts and covers follow
  deletions.
- **Data.**
  - `data/photos.json` is imported by `app.ts` and bundled.
  - The 240 unique JPEGs are copied to `assets/photos/` for the viewer.
  - `assets/thumbs/` holds 300 px centre-square thumbnails I made with `sips`, so the grid
    doesn't decode 1,200 px images.
  - The app icon is one of the photos.
- **Tests.** `app.test.contract` has 5 tests: the grid, viewer and favorite with relaunch,
  delete with relaunch, albums, and select mode.
  - `verify` (web) is clean, including dark mode and the largest text size.
  - `verify ios`: all tests pass, and there are no layout, dark-mode or large-text findings.

## What was hard / what I guessed

- **The pager.** A virtualized horizontal list needs a fixed literal `height`, which a
  full-screen pager can't give. So the strip is a plain horizontal `scroll` with one empty
  page box per photo, and only the pages within ±2 of the current one hold an image.
  - The initial position is `scrollLeft = index × pageWidth`.
  - The page width comes from `frame("app").width` when the photo opens.
  - The current page comes from the `scroll` event (`round(x / clientWidth)`).
  - Rotation isn't handled, so I set the device family to iPhone only.
- **Hiding the viewer's bars.** `navigationBar="none"` on a route that still had a `header`
  showed a bare system Back chevron. Removing the `header` as well, while the bars are
  hidden, gives a clean full-bleed screen.
- **Colours.** White hearts and checkmarks over photos, and the black backdrop, raised
  "literal colour" warnings. I wrapped those marks in a `color-scheme="dark"` subtree and
  used role colours, which keeps them light in both schemes. `color-scheme` can't be
  conditional ("light dark" is refused), so the black backdrop is a separate view that
  appears only while the bars are hidden.
- **Time format on iOS.** Apple's ICU puts U+202F before AM/PM, which broke text
  comparisons on iOS only. I normalized it to a plain space.
- **Day grouping is in UTC.** The data's timestamps are `Z` and the data module has no
  reliable device time zone, so day headings and times use UTC. That keeps them identical
  on every host. Photos uses local time.
- **Headings.** "Monday, Oct 26, 2026" style; there are no relative "Today"/"Yesterday"
  labels, since the data module can't read the clock.

## Not finished / known gaps

- `verify ios` still reports one scrolling finding: "glide after a still lift 196.7 pt". The
  feel test's slow drag pushes the content upward from where the list opens. This grid opens
  at the bottom (as the spec asks), so that drag is an overscroll, and the 197 pt is the
  rubber band springing back, not a real glide. I left it.
- Not built:
  - Pinch-zoom inside the viewer.
  - Drag-to-select across thumbnails.
  - Share and info.
  - A "Recently Deleted" album; deletions are permanent once confirmed.
  - Pinch to change the grid's column count.

## What would have made this faster

- A documented full-screen pager or carousel recipe (a paging strip of images with "current
  index" binding) — the fixed-height rule for virtualized horizontal lists isn't mentioned in
  the vocab entry for `scroll-snap-type`.
- A note in the docs that `navigationPresentation="fullscreen"` plus `navigationSource`
  gives UIKit's zoom and swipe-down dismiss. I found it by reading `ModalIOS.swift` and the
  `shared-elements` example; `contract vocab navigationSource` only says "prop, str".
- The scroll-feel check could account for lists that open at the end (`scroll-start="end"`).
