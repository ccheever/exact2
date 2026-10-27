# Photos (UIKit) — notes

## What's built
- **Library tab**: compositional-layout grid, 4 across with 2 pt gaps, square centre-cropped thumbnails, one section per local
  calendar day ("Today", "Yesterday", "Sun, Oct 25"; the year is added for other years). Oldest photo is at the top, and the
  grid opens scrolled to the bottom. A footer shows the photo count. White hearts mark favorites.
- **Viewer**: full-screen `UIPageViewController` of zoomable pages, each at the photo's own aspect ratio (double-tap or pinch to
  zoom). It is presented with the iOS 18+ `.zoom` transition, which gives the zoom from the thumbnail and the interactive
  swipe-down to close. The swipe is turned off while a photo is zoomed in. On close, the grid scrolls to whichever photo is showing.
  A single tap hides the bars and the status bar and turns the background black. The toolbar has Share, Favorite/Unfavorite and Delete
  (with confirmation). The title shows the day and the subtitle shows the time.
- **Albums tab**: the eight albums in a 2-column grid (1 column at accessibility text sizes), each showing its newest photo as the cover,
  its name and a count. An album opens the same grid, filtered, with "N Photos · M Favorites" in the footer. An empty album shows a
  placeholder.
- **Select mode**: Select/Cancel. The tab bar is swapped for a toolbar with Favorite (it becomes Unfavorite when every selected photo is
  already a favorite) and Delete (with confirmation). Selected photos show checkmarks. Two-finger or swipe multi-select is supported.
- **Persistence**: favorite changes and deleted ids are saved to `Application Support/library-state.json`, written atomically after every
  change. Deleted photos are removed from every grid, album count and cover.

## Testing
I ran it on the given simulator in light and dark mode and at AX-XXXL text size. `simctl` can't send taps, and trying to script the
Simulator with AppleScript just hung, so `AppDelegate.swift` (`DebugHooks`) reads launch arguments for testing
(`-debugOpen N`, `-debugPage ±N`, `-debugFav`, `-debugClose`, `-debugSelect N`, `-debugTab 1`, `-debugAlbum Pets`,
`-debugFavLast N`, `-debugDeleteLast N`). These drive the same code paths as the UI. I checked persistence across relaunches by
reading the state file and looking at screenshots.

## What was hard, and what I guessed
- The one real bug I found: when `collectionView.isEditing` is on, selection follows `allowsMultipleSelectionDuringEditing`, not
  `allowsMultipleSelection`, so at first select mode did nothing.
- I couldn't test the gestures themselves (horizontal swipe, swipe-down to close) with real touches. They come from the system
  (`UIPageViewController` and the zoom transition), and I tested the logic around them through the hooks above.
- Guesses: days are grouped in the device's time zone (timestamps are UTC). Photos with the same timestamp are ordered by id. Albums
  are sorted alphabetically. There is no "Recently Deleted" or Favorites album, because the spec says to list exactly eight albums.
  Deletion can't be undone.
- The 1,500 entries share 240 image files, so thumbnails are cached per file and decoded with ImageIO at 320 px on the short side.

## What would have made this faster
A way to send taps and swipes to the simulator (for example `simctl io tap`, or an allowed XCUITest runner).
