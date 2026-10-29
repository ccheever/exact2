# Feed — build notes

Completed and installed `Feed` (`dev.bakeoff.feed.b4dabcd`) on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final production app is running, with standard text size and light appearance restored after testing. No other simulator was used.

## Implementation

- All 400 posts and 120 JPEGs, plus their aspect-ratio metadata, are bundled under `Resources/data/`. The copies were verified byte for byte against `data/`.
- The lazy feed sorts by actual relative time, rather than file order. Full text, uncropped primary images, horizontally scrolling titled cards, likes, and reply counts are displayed.
- Native navigation opens post details. Images open in a full-screen viewer with downward-swipe dismissal, a close button, pinch zoom, double-tap zoom, and panning while zoomed.
- Pull to refresh adds three local posts. Compose supports Unicode, paragraphs, arbitrarily long text, whitespace validation, a focused text editor, and discard confirmation.
- Posts, likes, refresh state, and the original time anchor are saved atomically as JSON in the app’s Documents directory. Feed and detail views share one store.
- Adaptive colors, Dynamic Type, accessibility labels, like haptics, and a custom icon are included.

## What was hard

The supplied records are not strictly chronological, so sorting by their IDs or file order would have been incorrect. Keeping the initial time anchor in the save file also prevents relative times from resetting on relaunch.

The iOS 26 toolbar initially constrained the wordmark to a circular control. Explicit sizing and hiding its shared control background fixed it. Screenshot checks also caught a disabled detail header that dimmed text, carousel images widening their layout and clipping title beginnings, and composer placeholder truncation at the largest accessibility size. These were corrected and rendered again.

## Lookups and decisions

I used the skeleton README, the local Swift compiler, and `simctl` help; no web lookup was needed. Refresh authors and text, the local `You` / `@you` identity, initials avatars, the green-and-cream visual style, and the app icon were chosen locally because the spec leaves them open. Seed timestamps are calculated relative to the first launch; locally created posts use their creation date.

## Verification and limits

`Tests/` contains an isolated, XCTest-free simulator check app using production models and views. All 22 data checks passed, covering all 400 records, ordering, initial liked states, like/unlike counts, whitespace-only drafts, Unicode and paragraphs, long posts, repeated refresh, unique IDs, exact persistence, and decoding every referenced image. A separate actual process-relaunch check preserved composed posts, refreshed posts, seed likes, and new-post likes.

Simulator screenshots verified feed, detail, carousel, full-screen portrait photo, and the composer with its software keyboard. Dark mode and the largest accessibility size were checked, and the production app was rebuilt, installed, relaunched, and captured afterward. Evidence is under `build/`; test instructions are in `Tests/README.md`. The temporary check app was removed from the simulator.

No specified feature was intentionally omitted. Touch and gesture interaction could not be automated: the available computer-use connection did not approve access to Device Hub, and `simctl` provides no tap/swipe interface. The simulator checks therefore exercise production data actions and render production screens, rather than synthesizing user taps or swipes. Swipe dismissal and zoom are implemented but were not verified with synthesized gestures.

## What would have made this faster

An authorized simulator touch/accessibility driver would have allowed direct end-to-end interaction tests. A starter screenshot/test harness and an asset-catalog/icon pipeline would also have reduced setup time.
