# Photos (Exact) — build notes

Built with Exact. It is installed on simulator `CEDFB9C9-…` as `dev.bakeoff.photos.b3d6713` ("Photos").
`bun exact.mjs verify` and `verify ios` both pass: 5 tests; no findings in light, dark, both widths or the
largest text size; scrolling matches UIKit. The only warnings left are 6 `lower-colour-one-scheme`
ones, and they are deliberate (see below).

## What's there
- **Library**: a virtualized grid of all 1,500 photos, four across with 2 pt gaps. Thumbnails are square
  and cropped from the centre (`object-fit: cover`). The oldest photo is at the top, and the grid opens
  scrolled to the newest (`scroll-start="end"`). A heading such as "Monday, October 26" starts each day;
  the year is added only for days outside the newest photo's year. The subtitle shows the photo count.
- **Viewer**: a full-screen route (`navigationPresentation="fullscreen"`, zooming from its thumbnail on
  iOS 18 and later). Each photo is shown at its own aspect ratio (`contain`) on black. A sideways drag
  pages through the grid it was opened from (all photos, or one album). The page follows your finger,
  and a quarter-width drag or a fling turns the page. A drag down shrinks the photo and closes the
  viewer. A tap hides or shows the bars. The bars show the date, time, "N of M", a heart and Delete.
  Delete asks to confirm, then shows the next photo.
- **Favorites**: the heart in the viewer, or in bulk from select mode. A white heart marks favorites in
  the grid.
- **Albums tab**: the 8 albums in an inset-grouped list. Each row has a cover (the album's newest
  photo), the name and a count, and pushes the album's grid. The album grid has no day headings, as
  in Photos.
- **Select mode** (Library and album grids): tapping Select hides the tab bar. Tapped photos get a
  check badge. A bottom bar has Favorite (Unfavorite if every selected photo is already a favorite),
  "N Photos Selected" and Delete, which asks to confirm.
- **Persistence**: favorite changes and deletions are saved to `app:/data/library.json` and applied
  over the bundled `data/photos.json`. The open album and photo are also kept, so a relaunch into those
  screens shows the same album or photo.
- **Data**: `app.ts` imports `data/photos.json`. The 240 JPEGs are shipped in `assets/photos/`, plus
  300 px thumbnails I made with `sips` in `assets/thumbs/`. The grid never decodes 1,200 px images.

## What was hard
- **Viewer chrome in dark**: a native `header` in the full-screen route didn't take on
  `color-scheme="dark"`, so the title was black on black. I replaced it with the viewer's own overlay
  bars, as Exact's Excord viewer does.
- **The pager**: there is no paging-scroll primitive. I built it from `pan`/`panrelease`: the axis is
  locked on the first move, three keyed pages are shown (previous, current, next), and a spring
  `translate` animates the turn. Keying pages by photo id is what makes the turn animate smoothly
  when the index changes.
- **Glide after lift**: `verify ios` first reported a 197 pt glide after a still lift. I lowered
  `estimated-item-height` from 100 to the real average row height (85). Headings are about 52 pt and
  photo rows about 101 pt. After that, scrolling was "like UIKit". This came from guessing; nothing
  pointed at the cause.
- **Compiler errors**: the root component must be the file's first, and `xs[i]` doesn't exist (use
  `xs.at(i)` and `match`). A `for` in a virtualized list needs a single root element. The compiler
  explained each of these clearly.

## What I guessed
- I looked up these patterns in the pinned exact2 apps: `transformDragFor` and the full-screen viewer
  in `apps/excord/viewer.contract`, `navigationSource` zoom in `apps/shared-elements`, and swipe
  scrollers in `apps/messages`.
- Day labels use the device's time zone (`Intl` default). The data's timestamps are in UTC.
- Deleting is permanent; there is no "Recently Deleted". The app confirms first, so I didn't build one.
- The 6 fixed colours are deliberate: the viewer is always black, its bars are translucent black over
  the photo, and the heart, check badge and selection tint are white over photos. These match in both
  schemes, as in Photos.

## Not done / could be better
- No pinch-to-zoom in the viewer (not in the spec).
- No drag-to-select across thumbnails and no long-press menu.
- No sticky day headings.

## What would have made this faster
- A built-in horizontal pager (`list` with `paging=true`) with a dismiss gesture. This is the most
  common media-viewer pattern, and hand-building it was most of the work.
- A note in the full-screen route docs saying the native header ignores `color-scheme`, or a way to
  make it follow it.
- An app in the pattern list for a photo grid with a viewer.
