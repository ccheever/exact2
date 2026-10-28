# Feed — build notes

Completed and installed as `dev.bakeoff.feed.bf3c9f5` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A` using `EXACT_SIM=… bun exact.mjs ios`.

All requested features are implemented. The app bundles the original 400-post dataset, the image metadata, and all 120 JPEGs in `assets/data/`. It uses a virtualized feed, native navigation and composition sheets, image viewing with swipe-down dismissal, local refresh posts, and atomic file persistence for likes and new posts.

## What was hard

- Some carousels repeat the same image within one post. A filename alone is not a valid card identity. The final view uses the image/title pair, which is unique within every bundled carousel.
- A post-wide press target enclosing the interactive carousel triggered native overlap findings. Navigation now has distinct targets on the post body and its disclosure button; photos and likes have their own controls.
- Large text required wrapping metadata, responsive carousel widths, and breaking a word only when it exceeds the available line. Full paragraphs remain untruncated.
- The simulator’s real refresh gesture needed a held pull with platform timing. A real 350-point pull added exactly three posts. A real downward swipe dismissed the full-screen viewer.
- Editing a contract during a native build made one verification stale. The final verification ran against unchanged source.

## Lookups and decisions

I used Exact’s start guide, list-detail pattern, and vocabulary for virtualized lists, refresh, assets, file persistence, gestures, full-screen presentation, navigation, and simulator driving. No external services are used.

Bundled posts are ordered by `minutesAgo`, with the original id breaking ties. Relative times use compact minute/hour/day labels. Refresh fabricates three local text posts, and composed posts use “You” / “@you”. The carousel cards’ images use a consistent preview crop; opening one shows the complete image. Main post images preserve their supplied aspect ratios.

## Validation

- `bun exact.mjs verify`: clean; nine tests passed; light/dark and narrow/wide layouts passed; largest text checks passed.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: clean; nine tests passed; light/dark checks passed; nothing spilled at accessibility-extra-extra-extra-large.
- iPhone scrolling report: “like UIKit.” The final run judged physics only because the machine was busy; an earlier run reported nothing for the app to fix and identified remaining timing costs as framework-owned.
- Relaunch checks confirmed likes and composed posts persisted.
- Additional simulator drives exercised real pull-to-refresh and swipe-down photo dismissal.
- Data audit confirmed unique post ids, all referenced images present, and unique image/title card identities within each carousel.

Nothing requested remains unfinished.

## What would make this faster

A media-feed pattern combining a virtualized list, nested horizontal carousels, intrinsic image ratios, and a full-screen viewer would reduce vocabulary lookups. Earlier diagnostics for duplicate carousel keys and a documented native refresh gesture example would also help. Avoiding source edits during native verification prevents unnecessary reruns.
