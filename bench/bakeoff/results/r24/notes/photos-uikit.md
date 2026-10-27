# Photos (UIKit) — notes

## What's built
- **Library tab**: a 4-column grid (1.5 pt gaps) of square, centre-cropped thumbnails, oldest first. It opens
  scrolled to the newest photo at the bottom, with day headings ("Today", "Yesterday", weekday names for the past
  week, then "Thu, February 19") and a "1,500 Photos" footer. A heart marks favorites.
- **Viewer**: modal full screen with the iOS 18+ UIKit `.zoom` transition, which zooms from the tapped cell and
  gives an interactive swipe-down to close (turned off while you're zoomed in). A `UIPageViewController` pages left
  and right. Each page is a zoomable scroll view that fits the photo at its own aspect ratio, with double-tap to
  zoom. A single tap hides the chrome and turns the background black. The toolbar has favorite/unfavorite and
  delete (with confirmation). The title shows the date, with the time as a subtitle.
- **Albums tab**: an inset-grouped list of the 8 albums, each with a cover (newest photo) and a count. Tapping one
  pushes a grid of its photos. If an album is emptied it shows a "No Photos" empty state.
- **Select mode**: tap Select, or two-finger pan to multi-select. The tab bar hides and a toolbar with
  Favorite/Unfavorite and Delete appears. The title counts the selection. Long-press context menus offer the same
  actions.
- **Persistence**: favorites and deletions are written atomically to
  `Application Support/library-state.json` on every change. On first launch, favorites come from `photos.json`.

## What was hard or guessed
- Day boundaries use the device's local time zone, as Photos does. The data is in UTC, so a photo can fall on a
  different day than its UTC date suggests.
- Photos with the same `taken` time are ordered by id.
- Deleting is permanent; there is no Recently Deleted album. The spec only says "delete".
- Album order is alphabetical, and the cover is each album's newest photo.
- Only 240 image files back the 1,500 photos. Thumbnails are decoded once per file with ImageIO, downsampled,
  cropped square and cached.
- Opening at the bottom needed a second, deferred scroll, because self-sizing headers and footer change the content
  height at large text sizes.
- I couldn't test touch gestures: `simctl` can't tap or swipe. I added launch-argument hooks (`DemoHooks.swift`:
  `-demoTab`, `-demoAlbum`, `-demoOpen`, `-demoSelect`, `-demoAction favorite|delete`, `-demoHideChrome`) to drive
  screens and check persistence through screenshots. Swiping between pages and swiping down to dismiss are standard
  UIKit behaviour but were not exercised by hand.

## What would have made this faster
- A way to send taps and swipes to the simulator (an idb-style tool), or XCUITest support in the build script.
