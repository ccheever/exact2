# Photos (UIKit) — notes

## What was built
- `AppDelegate.swift`: a UIKit app delegate and scene delegate (the scene manifest is in `EXTRA_PLIST`), and a tab bar with Library and Albums.
- `PhotoStore.swift`: loads the bundled `photos.json` and sorts it oldest first (equal timestamps are ordered by id). Favorite overrides and deleted ids are saved to `Application Support/library-state.json` and broadcast to the screens through a notification.
- `ImageLoader.swift`: makes thumbnails with ImageIO downsampling at the cell's pixel size and loads full images off the main thread. Both are cached, and duplicate requests share one load (1,500 photos share 240 JPEG files).
- `GridViewController.swift`: one grid used for both the Library and each album. It is a compositional layout, 4 columns with 2 pt gaps and square cells cropped from the centre. Day headings read Today, Yesterday, the weekday for the past week, then the date. The grid opens scrolled to the newest photo. Select mode has Select All, a toolbar to favorite or delete together, a delete confirmation and dragging across photos to select them; the tab bar hides while selecting. A long press opens a context menu, and there are empty states.
- `PhotoViewerController.swift`: shown with the iOS 18+ `.zoom` transition, which gives swipe-down-to-close and zooms back to the photo's thumbnail even after paging. A `UIPageViewController` pages left and right. Each photo shows at its own aspect ratio and zooms by pinch or double tap; swipe-down is turned off while zoomed in. A single tap hides the bars and turns the background black. The bottom bar has Share, Favorite and Delete. After a delete the viewer moves to the next photo, and closes if none are left.
- `AlbumsViewController.swift`: an inset-grouped list of the 8 albums. Each row has a square cover (the album's newest photo), its name and a count, and works at accessibility text sizes. Counts and covers update live.

## Hard parts / guesses
- No tool for tapping the simulator was available (`osascript`/System Events timed out). I added `LaunchHooks.swift`, which only acts when the app is launched with `-hook ...`. It drives the app into the viewer, select mode, a batch favorite, a delete and an album so I could screenshot them. Every visual state was checked by screenshot (light, dark, AX5 text). Persistence was checked by relaunching. Swipe-down, horizontal paging and drag-to-select are standard UIKit behaviour but were **not** exercised by real touches.
- Guesses: days are grouped in the device's time zone. Albums are sorted alphabetically, and an album's cover is its newest photo. Batch favorite unfavorites the selection if every selected photo is already a favorite, and favorites all of them otherwise. Deletion is permanent; there is no Recently Deleted.
- I used `UINavigationItem.subtitle` (iOS 26) for the viewer's time and album line.

## What would have made it faster
- A way to send taps and swipes to the simulator (e.g. idb or `simctl io tap`) for end-to-end gesture testing.
