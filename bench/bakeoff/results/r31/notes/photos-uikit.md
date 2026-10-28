# Photos (UIKit) — notes

## What's built
- `Sources/App.swift`: UIKit app delegate and scene delegate (the scene manifest is in `EXTRA_PLIST`), and a `UITabBarController` using the iOS 18+ `UITab` API with two tabs, Library and Albums.
- `PhotoStore`: decodes `photos.json` and sorts it by `taken`, using `id` to break ties. Favorites and deleted ids are saved as JSON in Application Support. On first launch the favorites come from the data file. Changes are broadcast with a notification.
- `ImageLoader`: decodes ImageIO thumbnails off the main thread. Each thumbnail is sized so its *short* side is about 320 px, which keeps the centre crop sharp. Thumbnails and full images go in an `NSCache`, and duplicate requests are merged. The 1,500 photos share only 240 JPEG files, so the cache gets a lot of reuse.
- `GridViewController`:
  - Layout: compositional, 4 across with 2 pt gaps. Cells are pixel-snapped squares cropped with aspect-fill.
  - Library: one section per day, using the local calendar, with self-sizing day headings ("Today", "Yesterday" or "Thu, Feb 19", plus the year when it isn't this year). A "N Photos" footer ends the grid. It opens scrolled to the newest photo.
  - Albums reuse the same grid with no headings.
  - Select mode: the tab bar is hidden and a toolbar shows Favorite/Unfavorite (Unfavorite when every selected photo is already a favorite) and Delete, which asks for confirmation. The title shows the count. Two-finger pan multi-select is supported.
  - Long-pressing a thumbnail opens a context menu (Favorite, Delete) with a preview at the photo's own aspect ratio.
  - When there are no photos, an empty state is shown.
- `PhotoPagerViewController`:
  - Presented full screen with the iOS 18 `.zoom` transition, so the photo grows out of its thumbnail and swipe down (or pinch) closes it, shrinking it back into whichever cell matches the current photo. The grid scrolls if that cell is off screen.
  - Swipe left and right between photos (`UIPageViewController`). Each photo is fitted at its own aspect ratio, taken from the JSON so it is right even before the image loads, and supports pinch and double-tap zoom. Swipe-down dismissal is turned off while a photo is zoomed in.
  - Tapping hides the bars and turns the background black.
  - The title shows the date with the time as a subtitle.
  - Toolbar: Share, Favorite (heart, filled when on), and Delete. After a delete it moves to the next photo, or to the previous one if it was the last. Deleting the only photo closes the viewer.
- `AlbumsViewController`: 2 columns of album covers (the album's newest photo), each with its name and count. Counts update after deletions. At accessibility text sizes it switches to one column.
- Dark mode and Dynamic Type use system colours and scaled fonts throughout.

## Testing
`simctl` can't send taps, so I added `TestDriver`, which runs a comma-separated script passed with the launch argument `-drive`, e.g. `-drive "open:3,fav,next"` or `-drive "selectmode,select:1,select:5,deletesel"`. Without that argument it does nothing. `shot.sh name "script"` relaunches the app with a script and takes a screenshot. With these I checked:
- the grid, both ends, light and dark, at AX-XXXL text
- the viewer, favoriting, paging, hiding the bars
- select mode and the delete confirmation
- Albums and an album's grid
- that favorites and deletions survive a relaunch

I uninstalled and reinstalled at the end, so the installed app starts from the bundled data.

## What was hard / guessed
- **Opening scrolled to the bottom.** Self-sizing day headers change the content height after the first layout, which is most visible at large text sizes. The fix is to scroll to the bottom repeatedly until `contentSize` stops changing.
- **Swipe down to close.** This relies on the system zoom transition's interactive dismissal, and I could not drive a real pan gesture from the command line. The dismiss button, which uses the same transition, works. The swipe gesture itself is the one thing I could not verify directly.
- **Guesses:**
  - Album grids have no day headings, as in Photos.
  - The cover is the album's newest photo.
  - Albums are sorted by name.
  - Day grouping uses the device's time zone.
  - There is no "Favorites" album, because the spec says eight albums.
  - Deletion is permanent; there is no Recently Deleted.

## What would have made it faster
A way to send taps and swipes to the simulator (for example an `idb`-style tool) would have removed the need for the in-app test driver and allowed real testing of swipe-to-dismiss.
