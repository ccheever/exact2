# Photos (UIKit) — build notes

## What's here
- `Sources/App.swift` — UIKit app/scene delegates, tab bar (Library, Albums). Debug launch hooks are under `#if DEBUG`
  and are compiled out of the normal `./build.sh` build.
- `PhotoStore.swift` — loads `photos.json` from the bundle, sorts it oldest first (ties broken by id), groups photos into
  local-calendar days, and saves favorites and deletions to `Application Support/library-state.json` (atomic write
  on every change).
- `ImageLoader.swift` — ImageIO-downsampled, centre-cropped square thumbnails and full images, decoded off the main
  thread, cached in NSCache.
- `PhotoGridViewController.swift` — the 4-across grid (compositional layout, 2pt gaps, day headings in the Library,
  photo-count footer). It opens scrolled to the bottom. Select mode has Select All, a heart button (it unfavorites if every
  selected photo is already a favorite), and delete with a confirmation sheet. You can swipe sideways across photos to
  select them, with autoscroll near the edges.
- `PhotoViewerViewController.swift` — UIPageViewController paging between photos. Each page is a zoomable scroll view
  sized to the photo's own aspect ratio (pinch, double-tap). A single tap hides the bars and turns the background black.
  Pushing and closing use the iOS 18+ `.zoom` transition, which gives swipe-down-to-close back to the grid cell. That
  gesture is turned off while the photo is zoomed in. The viewer has favorite and delete buttons, and the title and
  subtitle show the date and time.
- `AlbumsViewController.swift` — the 8 albums in a 2-column grid (1 column at accessibility text sizes). Each shows a
  cover (the newest photo), its name and a count, and opens an album grid.
- `Resources/` holds a copy of `data/photos.json` and `data/images/`, so they ship in the bundle.
- `tools/devbuild.sh` (build.sh with `-D DEBUG`) and `tools/shot.sh` (relaunch with `-debugAction …`, then screenshot).

## What was hard / guessed
- `simctl` cannot tap or swipe, so I drove the screens through debug launch arguments (open the viewer, select mode,
  favorite or delete the selection, open an album) and checked screenshots. I could not test the horizontal swipe,
  swipe-down-to-close and swipe-to-select gestures by hand on the simulator. They use standard UIKit components
  (UIPageViewController and the zoom transition) plus a small pan recognizer.
- Only 240 image files serve 1,500 photos, so the thumbnail cache is keyed by file.
- Day headings use the device's local time zone, as Photos does. A photo near midnight UTC can therefore fall on a
  different day than its UTC date.
- Album order is alphabetical. The data has no album order, and the spec asks for exactly eight albums, so I did not add
  a "Favorites" album.
- Deleting removes the photo outright, with confirmation. There is no Recently Deleted album.

## What would have made it faster
- A way to send taps and swipes to the simulator (for example idb or an XCUITest runner) for end-to-end gesture tests.
