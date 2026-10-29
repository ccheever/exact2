# Photos (Exact): notes

Built with Exact and installed on simulator `CEDFB9C9-…` as `dev.bakeoff.photos.b49a337` ("Photos").
`bun exact.mjs verify` (web) and `verify ios` both pass all 6 tests, with no findings in light or
dark mode, and on iOS nothing spills at the largest text size. One finding is left (see below).

## What's there

- **Library tab**: every photo in a virtualized list. Each row is either a day heading
  ("Monday, Oct 26") or a 4-column CSS grid of square, centre-cropped thumbnails with 2 px gaps.
  The oldest photo is at the top, and the list opens at the end (`scroll-start="end"`). The
  subtitle shows the photo count.
- **Full-screen viewer**: a `fullscreen` route with `navigationSource`, so on iOS 18+ it opens
  with UIKit's zoom from the thumbnail and closes with UIKit's swipe down, which lands back on
  the thumbnail of the photo now showing. Each photo shows at its own aspect ratio
  (`object-fit: contain`). Swiping left or right moves one photo at a time. Tapping the photo
  hides or shows the bars. It has Favorite and Delete (Delete asks first), plus the date, time
  and album.
- **Hearts**: favorites get a white heart in the bottom-left of the thumbnail.
- **Albums tab**: a grid of the eight albums, each with a cover (its newest photo), name and
  count. An album opens a grid of its photos (no day headings, opening at the newest), which has
  its own Select.
- **Select mode** (both grids): Select/Cancel in the bar, a blue checkmark on each picked photo,
  and a bottom bar in the tab bar's place showing "N Photos Selected". It can favorite them (or
  unfavorite, if all are already favorites) or delete them after a confirmation alert.
- **Persistence**: `app.ts` reads the shipped `data/photos.json` (imported, so it is bundled) and
  keeps the edits in `app:/data/library.json`: favorites that differ from the shipped value, and
  the deleted ids. This survives a relaunch, which is tested.

## What was hard

- **The pager.** A horizontal virtualized `list` needs a literal pixel height and a literal
  `estimated-item-width`, so it can't fill a screen of any size. I used a plain snapping
  `scroll` instead, which builds only the current photo and its two neighbours, with spacers in
  percent widths for the rest. Snapping picks the snap point nearest where a flick would land,
  so building only three pages also keeps a fast flick from skipping photos. The index comes
  from the scroll event: `round(scrollLeft / clientWidth)`.
- **Opening the viewer at the right offset.** `scrollLeft` is pixels, so the tap measures the
  content panel (`frame("panel-library")`). On the web at wide sizes the tab bar becomes a
  sidebar, so measuring the root put the viewer one photo off. That turned up only in
  `verify`'s reload/resize checks.
- **Reopening on the viewer after a relaunch** came back with fresh state. The route now carries
  `/photo/:album/:id`, and a `task … when` rebuilds the viewer state from it.
- **Duplicate testIds.** Routes stay mounted, so the same `cell-6` existed in both the Library
  grid and the album grid, and a test tapped the wrong one. Album cells are now prefixed.
- **The native nav bar** stayed in the light scheme over the black viewer, so its title was
  invisible. The viewer draws its own bar (`header appearance="none"`).
- **On iOS a route fills the screen**, so a bar I placed after the content in the layout still
  covered the grid's last row. The grid adds bottom padding while selecting.

## Looked up or guessed

- I found the zoom transition and its swipe-down dismissal in Exact's Swift source
  (`ModalIOS.swift`, `ZoomPushIOS.swift`) and the `shared-elements` example app. The guide never
  says that a `fullscreen` route with `navigationSource` is how you get swipe-down-to-close.
- How scroll snap works on iOS: from `ScrollSnapIOS.swift`.
- **Guess: days use UTC.** The timestamps end in `Z`, and I kept days and times in UTC so the
  grouping is the same everywhere. Day headings are absolute dates ("Monday, Oct 26"), not
  Today/Yesterday.
- **Guess: album covers** are each album's newest photo.
- **Guess: no undo.** Deletion is permanent after the confirmation (there is no Recently
  Deleted album).

## Not finished or known gaps

- **`verify ios` scrolling finding**: "glide after a still lift 196.7 pt" on the Library grid.
  That list has no `scroll=` handler and no snapping, so by verify's own wording it is Exact's
  issue, not the app's. The machine was also overloaded (load 131), and verify did not judge
  timing for that reason.
- **No pinch-to-zoom** in the viewer (the spec doesn't ask for it).
- **No share button.**
- **Web only**: at the largest text size the authored web header's "Select" button clips. iOS
  draws its native bar and is clean.
- **Portrait only**: the iPhone app is locked to portrait (`orientation`), because the pager's
  starting offset is in pixels.
- **Thumbnails**: I made 360 px copies of the images (`assets/thumbs/`, made with `sips`) so 1,500
  grid cells don't decode full 1,200 px images. The full images are in `assets/images/`. The
  bundler won't follow a symlink, so the images moved out of `data/` into `assets/`, and I
  regenerated `images.json` there from `photos.json`.

## What would have made this faster

- A documented pager or paging scroll (`isPagingEnabled`), or virtualized horizontal lists that
  can size to their container.
- A line in the guide saying that a fullscreen route with `navigationSource` gives zoom plus
  swipe-down-to-dismiss.
- A warning when two mounted routes share a `testId`.
- A machine with free disk space: the volume was full at the start, so no commands could run for
  a while.
