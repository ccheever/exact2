# Photos (UIKit) — build notes

## What's here
- `Sources/AppDelegate.swift`: app and scene delegates, plus a `UITabBarController` built with the iOS 18+ `UITab` API (Library and Albums tabs).
- `Sources/PhotoStore.swift`:
  - The catalogue, sorted oldest first with ties broken by id (three photos share a timestamp).
  - Favorites and deletions, saved as JSON in Application Support. The starting favorites come from `photos.json`.
  - A thumbnail/full-image loader: ImageIO downsampling on a background queue, an `NSCache`, and de-duplication of requests already in flight. Only 240 image files back the 1,500 photos, so caching by file name works very well.
  - Date formatting.
- `Sources/GridViewController.swift`: the grid, used for both the library and each album.
  - Compositional layout: 4 across with 2 pt gaps, square cells with the image cropped from the centre.
  - Self-sizing day headings (Today, Yesterday, or "Sunday, October 25", with the year added for other years).
  - A count footer ("1,500 Photos · 80 Favorites").
  - Opens scrolled to the newest photo at the bottom.
  - Select mode: tap to select, or pan across thumbnails to select several. While selecting, the tab bar is hidden and a toolbar offers Share, Favorite/Unfavorite (it unfavorites only when every selected photo is already a favorite) and Delete (asks for confirmation first).
  - Shows an empty state when there are no photos.
- `Sources/PhotoViewerController.swift`: the full-screen viewer, a paging `UIPageViewController`.
  - Each photo is a zoomable scroll view at its own aspect ratio, with pinch and double-tap zoom.
  - It opens and closes with the system `.zoom` transition, which gives swipe-down-to-close. Swipe-down is turned off while you are zoomed in. If you've paged away, the source thumbnail is scrolled into view before the viewer closes.
  - Shows the date as the title and the time as the subtitle.
  - Tap to hide the bars and switch to a black background.
  - Share, favorite (red heart), and delete. After a delete it moves to the next photo, or the previous one at the end of the list, and closes when no photos are left.
- `Sources/AlbumsViewController.swift`: the eight albums in a 2-column grid (1 column at accessibility text sizes). Each shows its newest photo as the cover and its live count, and both update after deletes.
- `Resources/` holds copies of `data/photos.json` and `data/images/`. They are bundled as `photos.json` and `images/`.

## Testing
`simctl` can't tap, so `SceneDelegate.runDemoIfRequested` reads a launch argument that puts the app into a given state for screenshots:

```
xcrun simctl launch <udid> dev.bakeoff.photos.b133c58 -demo <state>
```

`<state>` is one of `viewer`, `chromeless`, `favorite-toggle`, `delete`, `albums`, `album`, `album-select` or `select`. `shoot.sh name [state]` relaunches the app in that state and takes a screenshot.

Checked by screenshot:
- the grid, opening at the bottom
- the viewer, with and without its bars
- a favorite saved to disk
- a delete surviving a relaunch (the count drops to 1,499)
- select mode in the library and in an album
- dark mode
- the largest accessibility text size

Afterwards I uninstalled and reinstalled the app so it starts with a fresh library.

## Hard, guessed, or not verified
- **Gestures weren't tested.** I couldn't test real swipes and pinches without touch input: interactive swipe-down dismissal, paging and drag-to-select. They rely on standard UIKit behaviour (`UIViewController.Transition.zoom`, `UIPageViewController`, the collection view's multiple-selection interaction).
- **Day boundaries** use the device's time zone (timestamps are UTC). That's what Photos does, but which day a photo near midnight falls on depends on the time zone.
- **Deleting** removes the photo outright, with a confirmation. There is no "Recently Deleted" album.
- **The Albums tab** lists exactly the eight albums in the data, alphabetically. There are no smart albums such as Favorites.
- **Landing at the bottom:** self-sizing headers make the first layout pass inaccurate, so the grid lays out and scrolls a few times before it lands exactly on the bottom.

## What would have made this faster
A way to send taps and swipes to the simulator (an `idb`/XCUITest-style tool). Without one I had to build launch-argument hooks to reach each screen state.
