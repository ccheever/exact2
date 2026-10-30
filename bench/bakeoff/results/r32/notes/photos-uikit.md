# Photos (UIKit): notes

## What's built
- **Library tab**: a compositional-layout grid, four across with 2 pt gaps. Thumbnails are square, centre-cropped and decoded off the main thread with ImageIO. Day headings ("Today", "Yesterday", "Sunday, October 25"; the year is added for other years). Oldest photo at the top; the grid opens scrolled to the newest at the bottom. Ties in `taken` are broken by `id`. The subtitle shows the photo count.
- **Viewer**: presented with the iOS 18+ `.zoom` transition, which zooms from and back to the thumbnail. That transition also supplies interactive swipe-down-to-close, and a down-swipe recogniser backs it up. It uses a `UIPageViewController` to swipe left and right. Each photo is fitted at its own aspect ratio (from `width`/`height`). Pinch or double-tap zooms. A single tap hides the bars and turns the background black. Heart toggles favorite; trash deletes after a confirmation. Deleting moves to the next photo, or to the previous one if it was the last, and closes the viewer when no photos are left. Title is the date, subtitle the time.
- **Favorites**: a white heart sits at the bottom-left of grid thumbnails. Long-pressing a thumbnail also offers Favorite and Delete.
- **Albums tab**: an inset-grouped list of the 8 albums. Each row has a cover (the album's newest photo) and a count, and opens that album's grid, which has the same features. An album whose photos are all deleted still shows, with a placeholder cover and an empty-state screen.
- **Select mode**: Select → tap photos (or two-finger / pan multi-select) → toolbar with Favorite (unfavorites if every selected photo is already a favorite) and Delete (confirmation sheet). The title shows "N Photos Selected" and the tab bar is hidden while selecting.
- **Persistence**: favorite overrides and deleted ids are saved as JSON in Application Support. They are applied on top of the bundled `photos.json` at launch.
- Dark mode and Dynamic Type up to AX5 checked on the simulator.

## What was hard / guessed
- `simctl` can't tap or swipe. To test, I added a launch-argument hook (`-UITestScript "open:last;prev;fav;chrome"`, see `DebugScript` in `AppDelegate.swift`) that drives the screens, then took screenshots. It does nothing unless that argument is passed.
- I could not physically test the swipe-down gesture. It relies on the zoom transition's built-in interactive dismissal plus a `UISwipeGestureRecognizer` fallback. Both are disabled while the photo is zoomed in.
- `NSCollectionLayoutGroup.horizontal(repeatingSubitem:count:)` with a `fractionalWidth(1)` item gave full-width items. I switched to absolute item sizes with flexible spacing.
- Guesses: albums are sorted alphabetically; the cover is the newest photo; day grouping uses the device's time zone; deletion is permanent (there is no Recently Deleted).
- The data reuses 240 images across 1,500 photos, so the thumbnail cache warms quickly.

## What would have made it faster
- A way to inject taps and swipes into the simulator (or XCUITest support in the build script).
