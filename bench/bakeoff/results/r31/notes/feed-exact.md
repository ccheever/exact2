# Feed — build notes

Finished and installed as **Feed** (`dev.bakeoff.feed.bedf676`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final `bun exact.mjs ios --run` succeeded and launched the app. No other simulator was used.

## Implemented

- All 400 supplied posts, ordered by their actual timestamps, with full text, author, handle, relative time, likes, and reply counts.
- All 120 original JPEGs, the original posts JSON, and image-size metadata bundled under `assets/data/`.
- Photos displayed at their supplied aspect ratios with rounded corners; horizontal image-and-title carousels.
- Native post navigation and a full-screen photo viewer with pinch zoom, double-tap zoom, close, and swipe-down dismissal.
- Pull-to-refresh adds three local posts. A toolbar refresh button provides an accessible alternative.
- A native compose sheet with keyboard handling, whitespace validation, Cancel, and Post.
- Likes, composed posts, refresh posts, and refresh sequence saved atomically in an app-local file. Writes are serialized to avoid losing concurrent changes.
- Native light/dark appearance, Dynamic Type, accessible controls, and a custom app icon.

## Hard parts and lookups

- The source file is not fully chronological. I sorted the initial posts by `minutesAgo`, preserving a deterministic order for ties.
- Several carousels reuse an image with a different title. Image filenames alone are not unique card identities. A data audit confirmed image/title pairs are unique, and the implementation keys cards by that pair. A regression test exercises repeated images with separate titles and actions.
- Large text required careful carousel layout and wrapping of words wider than the available line. The carousel itself is a horizontal flex scroller, and all prose stays complete.
- I looked up Exact's native navigation, asset bundling, storage grants, refresh control, transform gestures, and simulator-driving documentation. Some familiar CSS constructs are outside Exact's supported vocabulary; separating constraints and using the scroller's own row layout resolved this.
- The initial prebuild exposed an identity mismatch between the starter data module and the manifest. Both now use the required bundle identifier.
- The native refresh gesture needs a sufficient pull distance. Real-touch tests verified that each pull adds exactly three posts, including repeated pulls.

## Decisions

The supplied relative times are anchored to the first launch and stored as timestamps, so they continue aging across launches. Refresh posts use a rotating set of locally written updates. The app requires no network or account. The feed remains a single navigation column in wide test windows as well as on iPhone.

## Verification

- `bun exact.mjs verify`: **clean**, eight feature tests passed; light/dark, both widths, relaunch persistence, and largest text size passed.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: **clean**, eight tests passed, no layout findings, ten largest-text screens without spills, and no app-level scrolling findings.
- Native real-touch tests in `.exact/ios-gestures.test.contract`: **two passed**, covering swipe-down dismissal and pull-to-refresh.
- Bundled posts and every JPEG were checked against the supplied originals. The packaged bundle contains 400 posts and 120 JPEGs.
- Screenshots and verification reports are retained under `.exact/verify-web-final/` and `.exact/verify-ios-final/`.

No specified feature remains unfinished. The final iOS verifier recorded one 36.3 ms frame attributed to Exact itself, with nothing for the app to fix; its scrolling report is under `.exact/verify/ios-feel/`.

## What would have made this faster

A tested media-feed starter combining variable-height virtualization, horizontal carousels, bundled JSON/images, persistent likes, refresh, and a photo viewer would help. An identity preflight before native compilation, clearer examples of supported CSS constraints, and a documented real-touch refresh test would also shorten setup and iteration.
