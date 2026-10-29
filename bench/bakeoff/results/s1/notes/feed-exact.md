# Feed

Finished and installed as `dev.bakeoff.feed.b55e7f6` on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The final command was `bun exact.mjs ios --run --sim 486FD498-0E3E-4B64-8E4B-152BD8265EE7`.

## What works

- All 400 supplied posts, sorted newest first by `minutesAgo`, with full text, relative times, original-aspect-ratio images, and horizontal image/title carousels.
- Like toggles and counts, post detail navigation, and full-screen photos dismissed by a downward swipe or Close.
- Pull-to-refresh adds three locally invented posts each time. Compose rejects whitespace-only drafts and publishes multiline text at the top.
- Likes, refreshed posts, and composed posts persist in an atomically written local JSON file. A single queued mutation channel serializes writes.
- All supplied JSON and all 120 JPEGs are bundled under `assets/data/`. Every referenced image was checked in the installed bundle. The app has no network grant.

## What was hard and what I looked up

- Learned Exact's native navigation, modal routes, virtualized lists, gestures, asset bundling, storage grants, and typography from its guide, tested list-detail pattern, and vocabulary lookups.
- The supplied array is not chronological. Some carousels also repeat the same filename, so each card gets a stable separate identifier rather than using its image filename as its key.
- Accessibility checks caught fixed avatar bounds, overlong words, hanging whitespace, and undersized like-button hit areas. Avatars scale or disappear at accessibility sizes; post text wraps without truncation; line breaks remain intact; like controls have 44-point minimum hit areas.
- The initial native/compiler build required permission to write toolkit build caches. Editing sources while a native drive was starting also produced a stale-plan error; rebuilding the final unchanged revision resolved it.
- Real pull-to-refresh testing needed initial native layout to settle before pulling from the upper content area.

## Decisions and guesses

- Composed posts use the local identity “You” / `@you`.
- Refresh creates deterministic rotating text posts with invented authors. They require no network and remain saved like composed posts.
- Relative times use the dataset's supplied `minutesAgo`; new local posts display “Just now.”

## Validation

- `bun exact.mjs verify`: clean; eight tests pass, light/dark checks pass at phone and wide sizes, and the largest text size has no overflow.
- `bun exact.mjs verify ios --sim 486FD498-0E3E-4B64-8E4B-152BD8265EE7`: clean; eight tests pass, light/dark checks pass, and ten screens checked at the largest accessibility text size have no overflow.
- `bun exact.mjs test ios .exact/refresh.test.contract --sim 486FD498-0E3E-4B64-8E4B-152BD8265EE7`: passes. Two real pulls add exactly six posts; a like on a refreshed post and the added posts survive relaunch.
- Native scrolling found nothing for the app to fix. The machine was heavily loaded, so Exact judged scrolling physics rather than performance timing. It separately reported a toolkit-owned status-bar scroll-to-top timing difference.
- Only the requested simulator was used.

## Unfinished

No requested features remain unfinished.

## What would make this faster

A tested social-feed pattern covering nested horizontal carousels, photo viewing, multiline posts, and pull-to-refresh would reduce API discovery. Automatic stale-plan recovery and quicker warm native builds would also help. Exact's combined functional, dark-mode, accessibility, and scrolling verification was useful for catching the remaining polish issues.
