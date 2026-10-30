# Feed build notes

## Delivered

Feed implements the local feed, chronological fixture ordering, full post text, aspect-correct rounded images, horizontal image/title carousels, likes and reply counts, post navigation, a fullscreen photo viewer with swipe-down dismissal, pull-to-refresh with three local posts, and a multiline compose sheet. Likes, refreshed posts, and composed posts are saved with atomic file writes. Writes are serialized across all mutation types.

All 400 supplied posts, 120 JPEGs, and image metadata ship in `assets/data/`. All 659 image references resolve. The app also has a generated app icon.

## What was hard

- Combining a broad post tap target with independent like buttons, image taps, vertical scrolling, and horizontal carousel gestures. These were tested with real simulator touches.
- Making large text practical: decorative avatars disappear at accessibility sizes, author metadata stacks, post text remains complete, and the like control uses scalable content with a minimum 44-point target.
- Some supplied carousels reuse an image under different titles. Image filenames alone were insufficient keys. The final keys combine image and title; those pairs are unique in the supplied, immutable carousel data. A regression test covers a repeated-image carousel.
- Learning Exact's supported syntax for action props, transforms, asset bundling, and horizontal scrolling. Direct flex children on the scroller avoided incorrect inner-row content extents.

## Lookups and assumptions

I used the Exact start guide and list-detail pattern, and looked up resource/mutation typing, file grants and atomic writes, bundled assets, routes and modal/fullscreen presentation, live relative time, pull-to-refresh, pan gestures, and the simulator driver.

The compose identity is “You” / `@you`. Refresh uses a small rotating set of locally written authors and messages. Supplied `minutesAgo` values describe the fixture's age at launch; composed and refreshed posts store absolute creation times. Carousel images use a consistent crop, while each post's primary image uses its supplied native aspect ratio.

## Validation

- Nine native iOS functional tests cover ordering, like toggling and persistence, composing and whitespace validation, paragraph preservation, cancellation, refresh counts and persistence, post navigation, fullscreen images, swipe dismissal, all carousel cards, repeated-image card titles, and missing-post recovery.
- Real simulator touches checked pull-to-refresh, horizontal carousel scrolling, image dismissal, and composing while scrolled far down the feed.
- Native screen checks cover light and dark appearances and the largest Dynamic Type size.
- Native scrolling verification found nothing for the app to fix. The toolkit separately reports host-level status-bar scroll-to-top timing; that behavior is supplied by Exact.
- `bun exact.mjs verify` was also run during development. Its web checker reported clipped labels at the deliberately partially visible edge of a horizontal carousel; iPhone layout and gestures were checked directly.

## Limitations and verification configuration

No requested iPhone feature was left unfinished. One intentional layout notice is accepted in `app.json`: the clickable parent post contains a horizontally scrolling carousel whose next card is partly visible. Actual card taps and horizontal swipes work independently of the parent post target.

## What would make this faster

A tested social-feed starter with aspect-ratio media, nested horizontal carousels, and a fullscreen swipe-dismiss viewer would save substantial lookup time. More prominent guidance about scalable content inside custom social buttons, repeated-image keys, and direct horizontal-scroll children would also help. A warm native toolchain and an idle simulator host would shorten build and scrolling checks.

## Installation

- Name: `Feed`
- Bundle ID: `dev.bakeoff.feed.b9c434f`
- Simulator: `EF198926-1958-489D-A7A4-9196E454DD3A`
- Final standalone build installed using Exact's iOS command.

Final result: `bun exact.mjs verify ios` reported **verify: clean**, with 9 tests passed and nothing spilling at the largest text size. The standalone app was then built, installed, and launched successfully on the specified simulator with `bun exact.mjs ios --run`.
