# Feed — build notes

Finished and installed as `dev.bakeoff.feed.be6fbdb` (Feed) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final installed build is the normal build, with QA-only code excluded. Simulator appearance and text size were restored to light / Large.

## Implementation

- SwiftUI navigation, lazy scrolling feed, adaptive light/dark colors, and native sheets, editor, keyboard, and refresh control.
- All 400 supplied posts, all 120 JPEGs, and the image metadata are bundled. The 122 data files were checked byte-for-byte against the originals.
- Posts are sorted using their timestamps, since the input array is not chronological. Initial relative timestamps are anchored to the first launch and continue aging normally.
- Like state/counts and all locally created posts are saved atomically as JSON. The feed and detail screens share the same store.
- Refresh adds three locally authored posts on every pull. Compose rejects whitespace-only content, preserves multiline text, and keeps an unfinished draft across dismissal/relaunch.
- Main images preserve their supplied aspect ratios. Detail images are wider. Collection cards scroll horizontally and open their images full screen. The viewer includes swipe-down dismissal, an accessible close button, pinch zoom, and double-tap zoom.
- Custom local app icon; no network dependencies.

## Hard parts and decisions

The largest accessibility text size needed a separate, simpler composer header and supporting copy so that the keyboard does not crowd out the editor. The feed section heading also adapts at accessibility sizes. Native iOS 26 toolbar styling initially compressed the brand; hiding its shared toolbar background and giving it its natural width fixed that.

The specification leaves the current user and refresh content unspecified. I chose “You” / `@you`, six rotating local refresh templates, and deterministic initial avatars because no avatar files were supplied. Carousel thumbnails use a consistent crop, while individual post images and the full-screen viewer show the complete image.

No web lookup or downloaded libraries were needed. I read the project README and the computer-use skill and inspected the supplied JSON and image metadata. Compiler plugins and CoreSimulator needed execution outside the shell sandbox.

## Verification

`tools/test-simulator.sh` runs only against the assigned simulator, builds the QA configuration, runs isolated checks, captures screen layouts, restores appearance/text size, and reinstalls the normal build.

All 16 checks passed: 400-post loading, ordering, exact image aspect ratios and available files, titled collection cards, like/unlike counts and persistence, whitespace rejection, long multiline composition, exact three-post refresh batches, repeated refresh, unique IDs, persisted new posts, and an additional test across actual process termination/relaunch. Reports are in `build/qa-results.txt` and `build/qa-relaunch-results.txt`.

Simulator screenshots were visually inspected for the feed, post detail, collections, full-screen photo, composer with keyboard, dark mode, and the largest accessibility text size. Screenshots are in `build/`.

## Remaining limitations and what would help

All requested app features are implemented. Direct touch/gesture automation was not available: the desktop-control tool did not approve access to Device Hub. Consequently, real taps, swipes, pinch gestures, and VoiceOver traversal were not exercised end-to-end. Command-line simulator rendering and in-app data/persistence checks were used; these do not prove physical gesture behavior.

An approved simulator interaction tool with tap, drag, accessibility-tree inspection, and typing would make verification faster and more complete. A supplied current-user identity and refresh-content fixture would remove the small product assumptions above.
