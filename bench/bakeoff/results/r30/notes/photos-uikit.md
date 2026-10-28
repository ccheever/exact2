# Photos (UIKit): notes

## What's built
- **Library tab**: a compositional-layout grid, 4 across with 2 pt gaps. Thumbnails are square, centre-cropped and downsampled with ImageIO off the main thread, then cached in an NSCache. Photos are sorted by `taken` (ties broken by `id`; the JSON is not in order). Each local calendar day gets a heading ("Today", "Yesterday", "Sunday, October 25", with the year added for other years). A footer shows the photo count. The grid opens scrolled to the newest photo at the bottom, and a heart marks favourites.
- **Viewer**: full screen, each photo fitted at its own aspect ratio inside a zoomable scroll view (pinch, or double-tap to zoom). A `UIPageViewController` lets you swipe left and right through the photos. It opens and closes with the iOS 18+ `.zoom` transition, which also gives the system swipe-down to close; swipe-down is turned off while the photo is zoomed in. Tapping hides the bars and turns the background black. The toolbar has Favourite/Unfavourite and Delete with a confirmation sheet. The title shows the date and the subtitle the time.
- **Albums tab**: an inset-grouped list of the 8 albums. Each row has a cover (the album's newest photo) and a count. Tapping a row opens the same grid filtered to that album. Counts and covers update after deletions, and an empty album shows a placeholder.
- **Select mode**: tap Select, then tap photos (or two-finger-pan to select several). While selecting, the tab bar is swapped for a toolbar with Favourite (it switches to Unfavourite when every selected photo is already a favourite) and Delete (with confirmation). The title shows "N Photos Selected". A long press on a thumbnail opens a Favourite/Delete context menu.
- **Persistence**: the favourites set and deleted IDs are saved to `Application Support/library-state.json`. On first launch the favourites come from the JSON.
- Supports dark mode and Dynamic Type up to AX5: the headings wrap, and the opening scroll repeats until the self-sized headers settle.

## What was hard or guessed
- **Testing without touch**: `simctl` cannot tap or swipe. I added `-demo <name>` launch-argument hooks (`Sources/DebugHooks.swift`; they do nothing on a normal launch). With these and `shot.sh` I screenshotted the viewer, the hidden-chrome view, select mode, batch favourite, batch delete plus relaunch, albums and an album grid. Swipe paging and swipe-down to close use system components (page view controller, zoom transition) and were **not driven by real gestures**.
- When you set tabs with `UITabBarController.tabs` (`UITab`), `viewControllers` returns nil. The debug hooks had to keep their own references to the nav controllers.
- **Opening at the bottom**: the self-sizing headers make the first `contentSize` an estimate, so a single scroll fell short at large text sizes. It now loops until the offset is stable.
- **Guesses**:
  - Day headings use the device's local time zone.
  - Albums are in alphabetical order.
  - The cover is the newest photo.
  - Select mode ends after a batch action.
  - Deleting is immediate and permanent: there is no Recently Deleted.
- There are only 240 unique images for 1,500 photos, so caching by file name works well.

## What would have made it faster
- A way to send taps and swipes to the simulator (for example idb or an XCUITest runner), to test gestures for real.
