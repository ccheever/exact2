# Feed — implementation notes

Finished and installed as **Feed**, bundle ID `dev.bakeoff.feed.b85c10d`, on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. No other simulator was used.

## What ships

- All 400 original posts, sorted by `minutesAgo` ascending with a stable ID tie break. File order is not chronological; the oldest post is 391.
- The original data and all 120 JPEGs in `assets/data/`. All 122 bundled data files were checked byte for byte against `data/`.
- A virtualized native feed, full post text, aspect-correct rounded standalone images, horizontal image/title carousels, post details, and a full-screen dark photo viewer with swipe-down dismissal and an accessible close button.
- Like toggles and counts, reply counts, a native compose sheet, and pull-to-refresh adding three local posts. A toolbar refresh button provides another accessible way to refresh.
- Atomic local file persistence for added posts and like overrides. Writes are serialized so composing, refreshing, and liking cannot overwrite one another.
- Native navigation, semantic colors, Dynamic Type, and 44-point minimum interaction targets.

## Validation

- `bun exact.mjs verify`: clean, 12 tests passed. Largest web text size fits. A documented, narrowly scoped `app.json` exception covers the wide-web checker's reports that horizontally offscreen carousel cards are covered by the adjacent detail pane. Screenshots show correct clipping; tests reach the last cards.
- `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs verify ios`: clean, 12 tests passed, **zero native layout findings and zero accepted native findings**. Light/dark modes, the largest accessibility text size, persistence after relaunch, and the oldest post were checked.
- The native real-touch scroll probe reported UIKit-like physics, zero measured hitch time, and a 16.7 ms longest frame. It declined timing judgments because machine load was high.
- Additional touch-synthesizer drives confirmed native pull-to-refresh adds exactly three posts, composing/publishing works, and the oldest post opens. A full downward pull and settling were needed to reach UIKit's refresh threshold.
- Final installation command: `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs ios`.
- Screenshots and verification reports are under `.exact/verify/`; additional manual captures are under `.exact/`.

## What was hard / what I looked up

- Read Exact's required start guide, started from `list-detail`, then looked up images, virtualized lists, refresh controls, navigation presentation, gesture events, semantic colors/symbols, storage grants, and simulator driving.
- Pan deltas are incremental. Accumulating them was necessary for reliable swipe-down photo dismissal.
- Twelve supplied carousels repeat image filenames within a post. Each normalized card therefore has its own stable identifier derived from the post and immutable card position; filename keys would conflate distinct cards.
- A computed `calc(rem + px)` width compiled but was rejected by the native runtime. Runtime logs exposed it. Replacing the unnecessary track with a direct horizontal flex scroll removed the error and native overflow findings.
- Native verification requires an unchanged installed plan. Editing during an earlier run made later sessions reject a stale build; final verification ran against the installed final sources.
- Shared toolkit caches required sandbox escalation. Builds also printed an unused optional `wgpu-hal` patch warning; builds and installation succeeded.

## Decisions and assumptions

- Relative time uses concise minute/hour/day labels from the supplied snapshot; newly generated or composed posts say “Just now.”
- The local identity is “You” / `@you`. Refresh content cycles through locally authored updates; no network is used.
- Carousel photos may crop to their card image area; standalone post photos retain the exact supplied aspect ratio. Opening either kind shows the full image.
- Canceling a composer discards its draft. Published posts and likes persist, as specified.

## Anything unfinished

No known missing specification features. The only verification exception is the wide-web checker caveat described above; the shipping iPhone verification is clean without exceptions being used.

## What would make this faster

- Documentation that states prominently that pan deltas are incremental and demonstrates a complete photo viewer.
- A feed starter covering virtualized rows, carousels, pull-to-refresh, and persistence together.
- Reporting native invalid-style log entries directly in verification, especially when a bound value compiles but cannot be applied.
- Teaching the wide-web layout checker about clipped horizontal carousel descendants in split navigation.
- Read-only documentation commands that do not need a writable shared Cargo cache, and clearer separation between live agent plans and installed plans.
