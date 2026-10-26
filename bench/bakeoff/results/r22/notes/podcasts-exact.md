# Build notes

Implemented the complete specification in Exact: 420 bundled offline episodes, newest-first browsing and search, an unplayed filter, 12 shows with live episode/unplayed counts, episode summaries and Play/Pause state, direct list played/unplayed controls, and an Up Next queue with removal, drag reordering, and accessible earlier/later controls. Played changes, queue order, and the filter preference persist locally. Playback is a visual state, as requested; there is no audio.

## What was hard

- Learning Contract's differences from TypeScript: shaped resources, option-based array indexing, mapping then flattening, and action syntax. Precise compiler errors helped.
- Balancing native lists with large-catalogue performance. The final main feed and reorderable queue are virtualized; the shorter show and detail screens use native grouped lists. Exact does not combine grouped lists and virtualization.
- Making episode controls and metadata fit at the largest text size without reducing text. Rows wrap and move controls beneath the content at accessibility sizes.
- Native SF symbols can expand unexpectedly inside flexible columns. The detail decoration uses scalable dimensions and is omitted at accessibility sizes.

## Lookups and decisions

- Read Exact's required guide and the tabs pattern; looked up navigation, persistence, list rows, symbols, testing, and simulator driving.
- Imported the supplied JSON into the bundled data module for offline access. The original data file remains unchanged.
- Used persisted lists of IDs for played-state differences and queue order; metadata stays in the immutable catalogue. The bounded 420-item catalogue fits well within Exact's persistence limits.
- Formatted calendar dates in UTC so device time zones do not shift an episode's supplied date.
- Kept played episodes in Up Next until explicitly removed. Playing marks an episode played; pausing does not undo that status.

## Validation

- Eight interaction tests cover source data and ordering, list marking, filtering and persistence, live show counts, show navigation, episode playback and summary, queue ordering/removal/persistence, empty queues, and search.
- Web verification passes light/dark, narrow/wide, largest-text, reload, and relaunch checks.
- Real simulator touches successfully dragged an Up Next item to a new position. Largest-text queue checks pass in light and dark mode.

## What would make this faster

A compact reference covering Contract's TypeScript differences, a native feed/queue pattern, and clearer guidance on scalable symbol sizing and grouped-versus-virtual lists would reduce iterations. The shared toolkit Cargo cache also needs permission beyond this app's workspace.

All requested features are implemented. Native scroll timing is the remaining quality limitation; measurements and the optimization are documented below.

## Final native result and remaining limitation

Built, installed, and launched `Podcasts` (`dev.bakeoff.podcasts.b802a64`) using `bun exact.mjs ios --run` on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. No other simulator was used.

Native verification: 8 tests passed, 0 failed. All first-screen, test-end, relaunch, light/dark, and largest-text layout checks passed. The final horizontal-inset adjustment was checked separately on the simulator at normal and largest text sizes, and the complete web regression suite remained clean.

I could not make Exact's scrolling match UIKit on every timing metric. Switching the main catalogue from a grouped list to a virtualized feed reduced measured hitches from 40.9 to 5.8 ms/s and eliminated the initial 3-frame touch lag, dropped lift frames, and scrolling-offset jumps. The final full native audit still reported four timing findings: 5.8 ms/s hitches, a 46.1 ms longest frame, 0.56 momentum hand-off, and a 1.02-second status-bar scroll to top. Tracking, normal deceleration, bounce behavior, stopping, and blank-content checks passed. Exact's audit attributes momentum hand-off and status-bar timing to its host; the remaining frame costs are consistent with its renderer materializing these simple text-and-symbol rows. Those findings were retained rather than suppressed. Raw measurements are in `.exact/verify/ios-feel/feel.json` and `feel.txt`.
