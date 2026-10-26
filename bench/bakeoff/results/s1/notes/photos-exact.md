# Photos (Exact): notes

Built with Exact: `app.contract` is the view, `app.ts` the data module, and `app.test.contract` has six tests.
The app is installed on simulator `C2188CC6-…` as `dev.bakeoff.photos.be6e099`, named "Photos".

## What it does
- **Library tab.** One virtualized grid of all 1,500 photos, oldest at the top. Thumbnails are four across with 2 pt gaps, square and cropped from the centre. A heading separates each day: Today, Yesterday, the weekday for this week, then "Sat, Oct 18". Days are worked out in the device's time zone. The grid opens at the newest photo. A white heart marks favorites.
- **Viewer.** Opens as a full-screen route using iOS 18's zoom transition from the thumbnail. Swiping down dismisses it, using the zoom's own interactive dismissal, and it zooms back into the grid. Each photo is shown at its own aspect ratio (`object-fit: contain`).
  - The pager is a horizontal scroll that snaps to each page. Swipe left and right to page.
  - The title bar shows the day and the time.
  - The bottom bar has Favorite/Unfavorite and Delete (Delete asks first).
  - Tapping the photo hides the bars and turns the background black.
  - The viewer pages through whichever list you came from: the whole library or one album.
- **Albums tab.** The eight albums appear as a 2-column grid of square covers (the newest photo) with the name and count. At large text sizes it becomes one column. Each album opens a grid of its photos, scrolled to the newest.
- **Select mode** works in both the library and album grids. The header shows "N Photos Selected" and a Cancel button. Selected thumbnails get blue checkmarks. The tab bar is replaced by a bar with Favorite (or Unfavorite, if every selected photo is already a favorite) and Delete, which confirms with an alert.
- **Persistence.** Favorites and deletions are saved to `app:/data/library.json`, so they survive relaunching (tested with `reload`). The JSON that ships with the app is never changed.
- **Data shipped with the app.**
  - `data/photos.json` is imported by `app.ts`.
  - Full images are copied to `assets/photos/`.
  - 300×300 centre-cropped thumbnails were generated with `sips` into `assets/thumbs/`. Only 240 distinct images exist; the 1,500 records reuse them.

## What was hard / what I guessed
- **The pager.** A virtualized list's horizontal height and `estimated-item-width` have to be literal numbers, so a full-height virtual pager that sizes itself was refused. Instead I wrote my own window: up to five pages around the current one, with spacer views on either side so each page keeps its true offset. The scroll's `scrollLeft` opens at `index × width`, and the root's `resize=` handler keeps the width up to date.
- **"Open at the newest."** `scroll-start="end"` worked in the frozen-clock driver. On a real launch, though, the grid either mounted before the library arrived (so it opened at the top) or stopped about two rows short of the end. The fix has two parts:
  - Build the grid only once rows exist.
  - A `task … when lib.count > 0` runs `after(300)` and calls `scrollIntoView("grid-all", lastRowKey, block="end")`.

  I confirmed it with a cold `simctl launch` and a screenshot.
- **`Date.parse` vs manual parsing.** I first blamed `Date.parse` for the wrong ordering and switched to parsing the ISO string by hand. The real cause was the scroll position above, but the manual parsing is harmless, so I kept it.
- **The agent's clock.** The agent pins the clock to 2026‑01‑01 UTC, so its screenshots show "Mon, Oct 26" rather than "Today". A real launch shows Today and Yesterday correctly.
- **Colours over photos.** The heart and selection checkmark are always white over a photo. The colour lint wants roles, so I wrote them as `light-dark(#fff, #fff)`.
- **Small design guesses.**
  - Album order is alphabetical.
  - The album cover is the newest photo.
  - Album grids have no day headings (as in Photos).
  - Deleting is permanent: there is no "Recently Deleted" album.

## Not finished / known issues
- **`verify ios` still reports two scrolling findings.** It attributes both to Exact itself:
  - **Tracking ratio.** The first run had no `scroll=` handler on the grid and reported 0.16. The last run reported 1 ("ok").
  - **Status-bar tap.** Tapping the status bar does not scroll to the top, even though the route's `navigationScroll` names the grid.

  The machine was very busy during the runs (load average 29–136), and verify said timing was not judged. Main-thread time spent on images showed up as hitches. Shrinking the thumbnails to 300 px squares helped somewhat.
- No pinch-to-zoom in the viewer, and no share or info buttons.

## What would have made this faster
- A documented pager / `paging` primitive: a full-screen horizontal virtualized list with a computed height.
- `scroll-start="end"` behaving the same when rows arrive after mount on a real launch.
- A sample "photo grid + viewer" app. `shared-elements` was the closest and the most useful: its zoom route gave me the swipe-down dismissal for free.
