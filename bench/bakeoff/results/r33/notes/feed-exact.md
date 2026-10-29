# Feed build notes

## Delivered

Feed (`dev.bakeoff.feed.bfa6f63`) is built, installed, and running on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used.

Implemented the full local feed, likes, reply counts, post details, full-screen photos with swipe-down dismissal, horizontal image carousels, pull-to-refresh adding three local posts, and a native compose sheet. Likes and added posts persist in an atomically written local file. Writes are serialized so quick like, refresh, and compose operations cannot overwrite one another.

All 400 original posts and all 120 JPEGs are bundled. The original JSON and images were copied unchanged into `assets/data/`; all image references and byte-for-byte copies were checked. Image metadata determines standalone image aspect ratios.

## Validation

- Eight feature tests pass on iPhone and web, including blank compose validation, paragraph preservation, like toggles, detail and photo navigation, cancel, refresh, relaunch persistence, missing posts, and rapid writes.
- Final `bun exact.mjs verify ios` native report: `verify: clean`; no layout findings in light or dark mode, and nothing spills at the largest accessibility text size (53-point body) across ten screens.
- Real simulator touches confirmed pull-to-refresh and downward photo dismissal. Composing from a scrolled feed was tested with the software keyboard visible; publishing returns to the new top post.
- The scrolling check reports UIKit physics, zero measured hitch time, and a 16.7 ms longest frame. The verifier withheld its timing judgment because the shared machine was heavily loaded.
- Final installation used `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios --run`, which succeeded and launched the installed build.

The verifier's separate symbolic runner/Lean comparison still reports three divergences at boot, shrinking to zero-event cases. Consequently the complete verification command exits 1 after its clean native UI report. This symbolic result remains unresolved; it did not reproduce in the eight host tests or manual simulator checks. No requested app feature remains unfinished.

## Hard parts and lookups

- Used Exact's list-detail pattern, then looked up virtualization, native navigation, sheets, fullscreen presentation, gestures, refresh controls, local files, theme tokens, and delayed tasks.
- Fullscreen presentation was less discoverable than ordinary sheets; confirming the toolkit's native implementation helped.
- Dynamic Type exposed fixed-width initials and carousel-title problems. Author metadata wraps, card widths scale with type, and decorative avatars disappear at accessibility sizes. Oversized words use `overflow-wrap="break-word"` so the largest text size remains readable without reducing it.
- CSS transform strings are restricted by Exact; the photo drag uses its individual `translate` property. Delayed actions use a scheduled task rather than an action-level `after` command.
- Read-only documentation commands needed access to Exact's shared build cache. The initial native build also needed the toolkit's shared compilation caches.

## Decisions

Posts are ordered by `minutesAgo`, rather than input file order, because the source array is not strictly chronological. Ties retain source order. New local posts sort above all bundled posts. Bundled relative ages are displayed from their supplied values; new posts say “Just now.” Refresh authors and messages are a rotating local set, and composed posts belong to “You” / `@you`.

## What would make this faster

A tested photo-feed starter covering virtualized rows, horizontal carousels, local assets, fullscreen photo gestures, and accessible typography would save the most time. More prominent fullscreen documentation, cache-free documentation lookup, and separating native UI verification status from synthetic oracle failures would also help.
