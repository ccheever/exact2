# Feed build notes

Built with Exact for iPhone, using only simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` and bundle id `dev.bakeoff.feed.b41b6a5`.

The bundled 400-post dataset is sorted by `minutesAgo`, with stable id ordering for ties. The original JSON, image dimension index, and all JPEGs ship in `assets/data/`. Main photos retain their original aspect ratio. Carousel thumbnails use consistent crops; opening one shows the full image.

The hardest parts were nesting horizontal media strips in a long virtualized list, handling image gestures without triggering post navigation, and keeping long prose readable at the largest Dynamic Type size. Exact's action props infer parameters; its horizontal snap syntax accepts mandatory snapping. I looked up these details, native modal navigation, file grants, the simulator driver, and fixed media colors.

Design decisions: a native large-title feed, inline post navigation, a modal text composer, a dark full-screen photo viewer with a Close button and swipe-down gesture, native pull-to-refresh plus an accessible toolbar refresh alternative. Like changes and new posts are saved atomically in the local app data directory. The refresh generator cycles through local sample updates, with unique ids.

A toolkit cache outside the app directory required permission escalation. An in-workspace compiler cache, a directly documented refresh gesture test, and clearer handling of inactive routes in wide-window layout diagnostics would make this faster.

Validation includes bundled record count and order; like/unlike counts and relaunch persistence; full-screen image opening and swipe dismissal; compose whitespace validation, cancellation, posting, and persistence; refresh adding exactly three unique posts; reaching the oldest bundled post; and composing with the real software keyboard. Web checks are clean in both color schemes and at the largest text size. Native tests passed, largest Dynamic Type had no overflow, and scroll physics matched UIKit. Timing measurements were not judged because the build machine was busy.

Final result: `bun exact.mjs verify` is clean (7 web tests), and `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs verify ios` is clean (8 iPhone tests). Light/dark checks report no findings; all ten native screens checked at the largest text size fit. The final `bun exact.mjs ios` build completed and installed Feed on the assigned simulator. No specification items remain unfinished.

The toolkit emitted an unused optional `wgpu-hal` patch warning during packaging; it did not affect compilation, tests, or installation. No other simulator was used. The authoring diary remains local and unsent.
