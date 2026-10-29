# Photos (Exact): notes

Built with Exact from the `tabs` pattern. It is installed on simulator
`EEFC0029-1010-45E4-8242-42CF67E16C96` as `dev.bakeoff.photos.b3173bc` ("Photos").
`bun exact.mjs verify` and `verify ios` pass: 6/6 tests, no layout findings in light, dark or
at the largest text size, and the scrolling check found nothing the app can fix. The only
remaining findings are 5 compiler warnings about fixed colours. They are deliberate: the
heart and selection check drawn on top of photos stay white, and the viewer turns black
when its bars are hidden.

## What's there

- **Library**: all 1,500 photos in a 4-across grid with 2 px gaps. Thumbnails are square,
  cropped from the centre (`object-fit: cover` in a CSS grid with `aspect-ratio: 1`). Oldest
  is at the top. There is a heading for each day: Today, Yesterday, a weekday within the last
  week, then "Sat, Oct 24". It's a virtualized list with one item per day and
  `scroll-start="end"`, so it opens at the newest. The bar subtitle gives the photo count.
- **Viewer**: a fullscreen route using UIKit's zoom transition from the tapped thumbnail
  (`navigationSource`). Because of that transition, swiping down closes it natively, the same
  way Photos does. Each photo is shown at its own aspect ratio (`contain`). A horizontal
  snap-paging scroll moves between photos, newest to the right. Tapping hides the bars and
  turns the background black. The bar has the date, the time, Close, Favorite/Unfavorite (a
  heart) and Delete (asks first). On close, the grid scrolls the current photo into view so
  the zoom lands on its thumbnail.
- **Albums tab**: the 8 albums in a 2-column grid. Each shows a cover (its newest photo), a
  name and a count. At large text sizes it drops to one column. Tapping an album pushes a
  4-across grid of its photos, also opened at the newest, with a count at the foot. The viewer
  pages within that album.
- **Select mode** (Library and any album): Select → tap photos, which get a blue check →
  bottom toolbar (the tab bar hides) with Favorite and Delete. Delete asks for confirmation.
  Favorite turns every selected photo into a favourite, or unfavourites them all if every
  one already is one, as Photos does.
- **Persistence**: two `persist` states hold the ids whose favourite is flipped from the data
  and the deleted ids. Both survive relaunch; there's a test for each.

## What was hard

- **A paging viewer over 1,500 photos.** A virtualized horizontal `list` needs a literal
  numeric `height` (the compiler refuses `100%` or a measured value). So the pager is a plain
  snap `scroll` that I windowed by hand: a leading spacer of `lo × width`, the five pages
  around the current one, and a trailing spacer. The current index comes from
  `scroll=(x, y)`. The screen width is measured with `resize` on the root.
- **Select mode's toolbar.** At first I wrapped the tablist in `if not selecting`, which broke
  the native tab controller: the Albums panel showed instead of Library. The fix (from the
  docs) is `display="none"` on the tablist, never removing it.
- **Relaunching into a viewer route** opened a few photos away from the right one. The
  pre-measure width (390) and the real one (420 on the web) disagreed. Now the viewer finds
  its photo from the route's id, and a width change re-anchors it.
- **iOS's `Intl` puts a narrow no-break space before "PM"**, so the same text test failed on
  iOS but passed on the web. I normalised it in `app.ts`.

## What I guessed or decided

- **Time zone.** `taken` is in UTC, and its times run from 07:00 to 21:00. I treated it as the
  wall-clock time the photo was taken, so days and times are shown as written and not shifted
  into the device's zone. Shifting into the simulator's US zone would have split mornings
  into the previous day.
- **Album order** is alphabetical. The cover is the newest photo, as in Photos.
- **Thumbnails.** I added downscaled copies (540 px on the long side) in
  `assets/thumbs/`, generated with `sips`, so the grid doesn't decode 1,200 px JPEGs. The
  original images ship in `assets/photos/` and the data in `assets/photos.json`. `app.ts`
  imports the JSON.
- **The data has only 240 unique image files** for the 1,500 photos, so images repeat. That's
  expected.
- **Extras beyond the spec**: delete from the viewer, tap to hide the bars, and the photo
  count in the bar.

## Not done

- No pinch-to-zoom inside the viewer, and no drag-to-select-many in the grid.
- No Favorites smart album; the spec says eight albums, so I left it out.
- The viewer's route URL keeps the id of the photo it opened on, not the one you swiped to.
  The viewer itself tracks the current photo.

## What would have made it faster

- A "pager" or photo-viewer pattern. Paging horizontally, with a current index, across a
  large collection is common, and the virtualized-list height rule pushed me to hand-roll
  the windowing.
- Putting "hide the tab bar with `display="none"`, never `if`" in the tabs pattern or the
  card. I found it only after hitting the bug.
- A note that a `fullscreen` route with `navigationSource` gives the interactive swipe-down
  dismissal for free. I expected to build it with `pan`, and only found out by trying.
