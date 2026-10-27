# Feed build notes

Feed (`dev.bakeoff.feed.b1a83a9`) is built, installed, and launched on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

All requested features are implemented: the complete local feed, likes, reply counts, post details, full-screen photos with swipe-down dismissal, horizontal carousels, pull-to-refresh adding three posts, composing, and persistence.

## Data and implementation decisions

- All 400 supplied posts and all 120 images ship with the app. The original JSON and image metadata are included under `assets/data/`; `seed.ts` normalizes the initial records for Exact's data module.
- The supplied posts were not strictly chronological. Initial ordering uses `minutesAgo`, with ID as the tie breaker.
- Main images use their supplied aspect ratios. Carousel previews use consistent cropped thumbnails; opening a thumbnail shows the complete original image.
- There were no author portraits, so avatars use initials. New text posts are authored by “You.” Refresh creates three local posts with unique IDs using a small rotation of locally written messages.
- Likes, composed posts, and refresh additions are written atomically. All mutations share a write queue so overlapping refreshes and likes cannot overwrite one another.

## What was hard and what I looked up

- Exact's navigation, fullscreen presentation, asset bundling, filesystem grants, virtualized lists, and simulator-driving APIs required documentation lookups.
- Horizontal carousels needed explicit content bounds. Exact supports a narrower CSS length vocabulary than browsers; `max-content` and `rem` inside `min()` were unavailable here.
- Pan events report incremental movement. Real-touch testing caught the initial mistake of treating each delta as the full swipe distance.
- Twelve supplied carousels repeat an image filename. Cards therefore need their own stable IDs, rather than using the image filename as a key. A regression test covers repeated images with different titles.
- Composed line breaks required explicit whitespace handling. `pre-line` preserves paragraphs and wraps cleanly at the largest text size.

## Validation

- Nine authored tests passed on both web and iOS.
- `bun exact.mjs verify`: clean, including light/dark, relaunch, two widths, and 53 px accessibility text.
- `bun exact.mjs verify ios`: clean, including native sheets, full-screen viewer, relaunch, and the largest iOS accessibility text size.
- Real simulator touches verified photo dismissal, the keyboard and composer, and pull-to-refresh. A refresh produced 403 posts; a separate relaunch retained them. A later multiline post and like also survived another relaunch.
- Every bundled image reference, all 400 post IDs, chronological ordering, and every carousel card identity were checked.
- Final installation used `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`.

No requested feature remains unfinished. Exact's scrolling qualification reported no app-level issue; it did identify framework-level frame/scrubbing timing issues outside this app's code.

## What would have made this faster

A tested social-feed starter with aspect-ratio images, horizontal carousels, and Dynamic Type examples would help. More prominent documentation of incremental pan deltas and a native refresh test command would also shorten the simulator debugging loop.
