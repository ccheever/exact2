# Photos (UIKit) — notes

## What's built
- **Library tab**: a compositional-layout grid, four across with 2 pt gaps. Thumbnails are square and cropped from the centre (ImageIO downsampling plus aspect-fill). The grid runs oldest to newest, with one section per local calendar day and a day heading ("Today", "Yesterday", "Monday, October 26", or with the year if it isn't the current one). It opens scrolled to the newest photo, and a "1,500 Photos" footer sits at the bottom.
- **Viewer**: a `UIPageViewController` of zoomable pages. Each photo is shown at its own aspect ratio, and you can pinch or double-tap to zoom. It opens with the iOS 18+ `.zoom` transition from the tapped cell, which gives Photos-style interactive swipe-down to close. Swipe-down is turned off while a photo is zoomed in. There is also a Close button. A single tap hides the chrome and turns the background black. A heart button favorites or unfavorites the photo, and a trash button deletes it after a confirmation; the viewer then moves on to the next photo.
- **Hearts** mark favorites on grid cells.
- **Albums tab**: a two-column grid of the eight albums (one column at accessibility text sizes). Each shows its newest photo as the cover, plus its name and count, and opens a grid of that album's photos. Albums whose photos have all been deleted show a placeholder.
- **Select mode** (in the library and in album grids): Select / Cancel / Select All. Tap to select, or drag with two fingers. The tab bar is swapped for a toolbar with Favorite and Delete. Favorite sets all selected photos to favorite, or unfavorites them if they all already are. Delete asks for confirmation first.
- **Persistence**: favorite overrides and deleted ids are written atomically to `Application Support/library-state.json` on every change. I checked that they survive terminate and relaunch.
- I checked dark mode and the AX5 text size with screenshots.

## Hard / guessed
- `simctl` can't send touches, so I added launch-argument hooks (`-debugOpen N`, `-debugSelect N`, `-debugAction favorite|delete`, `-debugAlbum Name`, `-debugTab N`) to reach each state for screenshots. They do nothing in a normal launch.
  - **Not tested:** the interactive gestures themselves (swiping between photos, swiping down to close, drag-selecting). These come from system components (`UIPageViewController`, the zoom transition, the collection view's multi-select).
- The first select-mode test selected nothing. A collection view in editing mode needs `allowsSelectionDuringEditing` and `allowsMultipleSelectionDuringEditing`; without them, real taps would also have failed.
- The albums layout overflowed: an item width of `fractionalWidth(1)` with `repeatingSubitem:count:` doesn't split the row into columns. I fixed it by computing absolute widths.
- **Guesses:**
  - Day headings use the device's time zone (the timestamps are UTC).
  - Library photos with the same timestamp are ordered by id.
  - Album grids have no day headings.
  - Albums are sorted alphabetically.
  - There's no "Recently Deleted"; a deletion is permanent but asks for confirmation first.
  - The 1,500 photos share 240 image files, so thumbnails are cached per file.

## What would have made it faster
A way to inject taps and swipes on the simulator (or an XCUITest runner in the skeleton), so the gesture features could be checked end to end.
