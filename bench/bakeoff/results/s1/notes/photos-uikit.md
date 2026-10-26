# Photos (UIKit) — notes

## What's built
- `AppDelegate.swift`: a UIKit app delegate and scene delegate (the scene manifest is in `EXTRA_PLIST`), plus a tab bar with Library and Albums.
- `PhotoStore.swift`: loads `photos.json` from the bundle and sorts it oldest first (ties broken by id). It saves favorites and deletions to `Application Support/library-state.json` and posts a change notification.
- `ImageLoader.swift`: decodes images off the main thread with ImageIO and caches them in memory. Grid thumbnails are centre-cropped squares at the cell's pixel size; the viewer gets full images.
- `PhotoGridViewController.swift`
  - Four-column compositional layout with 2 pt gaps.
  - Day headings ("Today", "Yesterday", "Saturday, October 24") and a "1,500 Photos · 80 Favorites" footer.
  - Opens scrolled to the newest photo at the bottom.
  - Long-press menu with Share, Favorite and Delete.
  - Select mode: tap, drag sideways to select a range (scrolls automatically at the edges), two-finger pan, and Select All. A toolbar offers Share, Favorite/Unfavorite and Delete (with a confirmation). The tab bar hides while selecting.
  - Shows an empty-state screen when there are no photos.
  - The same controller is used for album grids, which have no day headings.
- `PhotoViewerController.swift`
  - Full-screen paging viewer that shows each photo at its own aspect ratio.
  - Pinch or double-tap to zoom; tap to hide the bars and turn the background black.
  - Title shows the date and time.
  - Share, Favorite and Delete buttons. After a delete it moves to the next photo, and closes if none are left.
  - Uses the iOS 18+ `.zoom` transition: it zooms out of the grid cell, and swipe-down to close comes from the system. Swipe-down is turned off while the photo is zoomed in.
- `AlbumsViewController.swift`: the 8 albums in a two-column grid (one column at accessibility text sizes). The cover is the album's newest photo, with the count underneath. Tapping an album opens its grid.
- Data: `data/photos.json` and `data/images/` are copied into `Resources/` so they ship in the app bundle.

## Testing
- Checked on the simulator with screenshots: grid, viewer, albums, album grid, select mode, dark mode, the largest accessibility text size, and favorites/deletions surviving a relaunch.
- `simctl` cannot send taps or swipes, so I added launch-argument hooks (`DebugDriver` in `AppDelegate.swift`): `-debugOpenPhoto <id>`, `-debugAlbum <name>`, `-debugTab 1`, `-debugSelect <n>`, `-debugFavorite <id>`, `-debugDelete <id>`. `shot.sh` relaunches the app with arguments and takes a screenshot.
- **Not tested by hand: any gesture.** That covers swiping between photos, swipe-down to close, the zoom transition, drag-to-select, and the context menu. They rely on standard UIKit behaviour (`UIPageViewController`, the zoom transition's interactive dismiss, `UIScrollView` zooming) but I never saw them run.
- I deleted the test state afterwards, so the app on the simulator starts from the bundled data.

## Hard parts and guesses
- **Data quirks:** 1,500 photos reuse 240 image files, and three photos share a timestamp with another photo. Caching thumbnails by file name helps a lot with the repeats.
- **Scrolling to the bottom on open:** the day headings size themselves to the text, so the content height only settles after layout. The grid re-runs layout and pins to the end a few times.
- **Time zone:** days are grouped in the device's local time zone (timestamps are UTC), as Photos does.
- **Guesses:**
  - Albums are sorted alphabetically.
  - The cover is the newest photo in the album.
  - Deleting is permanent, with no Recently Deleted album.
  - Favoriting a selection where every photo is already a favorite unfavorites them instead.
  - Select mode ends after a favorite or delete.
- **Swift version:** swiftc is version 6.4, but the build runs in Swift 5 language mode, so the code avoids the strict-concurrency rules.

## What would have made this faster
- A way to send touches to the simulator (an idb-style tap/swipe command), or an Xcode project to run XCUITests. I couldn't check any gesture-driven behaviour directly.
