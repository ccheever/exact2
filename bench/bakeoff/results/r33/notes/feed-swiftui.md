# Feed — build notes

Feed is built, installed, and running on simulator `EF198926-1958-489D-A7A4-9196E454DD3A` as `dev.bakeoff.feed.b046c92`. The final installed build uses the regular 400-post feed, without verification posts or screen-routing hooks enabled.

Implemented all six requested features: chronological local feed, complete text and aspect-correct images, titled horizontal carousels, persistent likes, post detail, full-screen photos with swipe-down dismissal, three-post refresh, and text composition. The photo viewer also supports pinch/double-tap zoom and bounded panning. All 400 seed posts, all 120 JPEGs, and their size metadata are bundled under `Resources/data/`. No network access or third-party dependencies are used.

## What was hard

- The input array was not chronological. Posts are sorted by their age, with stable dates established on first launch.
- A rapid-refresh persistence check found that fractional timestamps could reorder a new batch after relaunch. Each batch now shares a timestamp and has a deterministic ID order.
- The largest Dynamic Type size required simpler headings, fitting decorative initials inside their avatars, more keyboard/editor space, and a simpler photo toolbar. Full post text and card titles remain unrestricted.
- Image decoding is cached with a memory limit, and the vertical feed is lazy. Main images keep their source aspect ratio; carousel thumbnails use a consistent crop.

## Lookups and decisions

Read the provided README, inspected the post/image JSON, and consulted local `simctl` help. Compiler feedback resolved SwiftUI’s generic text-selection modifier. No web research was needed.

The local composing identity is “You” / `@you`. Refresh content rotates through nine locally written examples, adding exactly three new post records each time. Seed ages continue advancing after relaunch. Empty or whitespace-only posts cannot be published; internal paragraphs and emoji are preserved, with no arbitrary character limit. Cancelling a nonempty draft asks before discarding it.

## Verification

- `Testing/ModelChecks.swift` passed: 400 seeds, chronological ordering, both initial like states, like/unlike count changes, blank/long composition, repeated refresh, unique IDs, persistence of text/counts/order, and relative-time boundaries.
- Verified all 659 image references resolve, and bundled source data/images match the supplied files.
- Launched and inspected actual simulator renders of feed, portrait and landscape detail, carousel, full-screen viewer, and compose with its keyboard. Checked light/dark appearances and the largest accessibility text size.
- Simulator process relaunches preserved a composed post, three refreshed posts, and a liked post with its incremented count. Verification used a separate persistence file.
- Built and installed the final normal build using `SIM=EF198926-1958-489D-A7A4-9196E454DD3A ./build.sh --run`. Confirmed bundle identity, all bundled assets, valid signing, successful relaunch, and exclusion of verification-only strings. Final screenshot: `build/finished.png`.

The optional `SWIFT_FLAGS=-DFEED_TESTING` build flag enables isolated screenshot scenarios via `--scenario`; it is disabled in the installed app. The model checks can be rerun with:

```sh
xcrun swiftc -module-cache-path build/ModuleCache Sources/Models.swift Testing/ModelChecks.swift -o build/model-checks
build/model-checks
```

## Remaining limitations and what would help

No requested app feature was intentionally omitted. Direct tap/swipe/pinch automation was unavailable: computer-use access to the Device Hub surface was not approved, and `simctl io` exposes capture rather than touch input. Gesture handlers were reviewed; screen presentation, model actions, and relaunch behavior were checked through compile-time simulator scenarios rather than end-to-end touch automation.

An authorized input driver for this specific simulator would have made gesture integration testing faster and stronger. A starter with isolated screenshot launch scenarios and an explicit compiler-flags option would also have saved setup time.
