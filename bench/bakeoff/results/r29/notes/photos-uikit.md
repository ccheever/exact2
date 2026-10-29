# Photos (UIKit): notes

## What's built
- **Library tab**: a compositional-layout grid, 4 across with 2 pt gaps. Thumbnails are square, cropped from the centre and decoded off the main thread with ImageIO (thumbnail sized so the short side fills the cell, then cropped), and cached in an NSCache. Day headings read Today, Yesterday, a weekday name within the past week, or otherwise "Sat, Oct 18" (with the year if it isn't this year). Photos are sorted oldest first, with ties on `taken` broken by `id`; three timestamps in the data are duplicated. The grid opens scrolled to the newest photo, and the subtitle shows the photo count. A white heart marks favorites.
- **Viewer**: a `UIPageViewController` of zoomable pages. Each photo is fitted at its own aspect ratio; double-tap or pinch to zoom. Swipe sideways to page. Opening and closing use the iOS 18+ `.zoom` navigation transition, which gives an interactive swipe-down (and pinch) to close that lands back on the current photo's thumbnail. The grid scrolls that thumbnail into view first if it is off screen. Swipe-down is blocked while the photo is zoomed in. Tap hides the bars and turns the background black. The toolbar has Share, Favorite/Unfavorite and Delete (with a confirmation). After a delete the viewer moves to the next photo, or closes if none are left.
- **Albums tab**: an inset-grouped list of the 8 albums. Each row has the newest photo as its cover and a count that leaves out deleted photos. Tapping a row opens the same grid filtered to that album. An album with no photos left shows a placeholder.
- **Select mode**: Select/Cancel in the nav bar. The tab bar is swapped for a toolbar with Favorite (it becomes Unfavorite when every selected photo is already a favorite) and Delete (with a confirmation sheet). You can tap photos, drag one finger sideways to paint a selection (it auto-scrolls at the edges), or use the system two-finger pan. Long-pressing a photo outside select mode opens a context menu with Favorite and Delete.
- **Persistence**: favorites (the full set, seeded from the JSON) and deleted ids are saved in UserDefaults. Every grid, the album list and the viewer watch for changes through a notification.
- Uses only system colours and Dynamic Type fonts, so dark mode and accessibility text sizes work. I checked AX3 on the simulator.

## Hard / guessed
- `UIZoomTransitionOptions.InteractionContext` has no `zoomedViewController` (the source-view provider context does), so I captured the pager weakly instead.
- `repeatingSubitem:count:` with a fractional-width item made every cell full width. I switched to absolute item sizes and spread the leftover fraction of a point into the gaps.
- Day grouping uses the device's local time zone, the way Photos does. The JSON times are UTC, so a photo near midnight UTC can fall on a different day than its date string suggests.
- I chose to list the albums alphabetically, use the newest photo as the cover, and show a plain count as Photos does.
- I couldn't drive touches on the simulator, since only `simctl` is available. Swipes, the zoom transition's interactive dismissal and drag-to-select are written against documented system behaviour, but I never exercised them by hand. Screens were checked through `-debugOpen <id>`, `-debugAlbums`, `-debugAlbum <name>` and `-debugSelect` launch arguments, which are left in the app (harmless). Persistence was checked by writing deletions from the app, relaunching, and seeding UserDefaults by hand.

## Would have made this faster
- A way to inject taps and swipes into the simulator (for example idb, or an XCUITest runner) to test the gestures.
