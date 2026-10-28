# Build notes

Implemented all seven requested features in Exact: 420 episodes in date order, an unplayed filter, 12 shows with live counts, episode details and simulated playback, list actions for played state, and a persistent, reorderable Up Next queue.

The original `data/episodes.json` is imported by the data module and also shipped unchanged as `assets/episodes.json`. The data module gives dates an explicit UTC interpretation and makes ties deterministic by episode ID. Initial played flags come from the supplied file; saved ID overrides preserve subsequent changes. Queue IDs and their order use Exact's persisted root state, well within its 16 KB limit even with all 420 episodes queued.

What was hard:
- Learning Exact's distinction between grouped lists and virtualized feeds. Grouped appearance cannot be combined with virtualization.
- Native large-text testing caught a fixed segmented filter overlapping the feed. Moving the filter to a native navigation menu removed the overlap and kept it available on empty results.
- Queue rows needed a smaller flexible text basis so the More button and drag handle fit beside the content at normal text sizes, while wrapping at accessibility sizes.
- The first native toolkit build took several minutes; later changes reached the installed app quickly.

Lookups and decisions:
- Used Exact's tabs starter, then looked up component props, persistence, native menus, virtualized lists, reorderdrop/reorderFor, bundling, and simulator driving.
- Chose UTC date formatting for the date-only source values to avoid time-zone date shifts.
- Kept queue membership when an episode plays, since removal after playback was not specified. Play marks an episode played immediately; Pause keeps that history. The Now Playing indicator is intentionally session-only, with no audio.
- Provided Move Up and Move Down actions as alternatives to drag handles, plus menu removal and swipe deletion.

Validation:
- `bun exact.mjs verify`: seven tests passed, clean light/dark layouts at phone and wide widths, and no spills across 14 screens at the largest text size.
- Native real-touch drive added two episodes and dragged the first below the second; the queue changed from [177, 365] to [365, 177].
- Final iPhone verification and install results are appended below.

Nothing in the requested feature set was intentionally left unfinished. No audio was added, as specified.

A ready-made native feed example with a filter menu and reorder handles, plus vocabulary aliases for “reorder,” would have made this faster. The compiler's suggested corrections and built-in verification were useful.

Final results:
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs verify ios`: **clean**. Seven tests passed; all native light/dark checks passed; nine screens at accessibility-extra-extra-extra-large had no spills. Scrolling physics matched UIKit. Machine load was high, so the toolkit did not judge absolute frame timing.
- `EXACT_SIM=EF198926-1958-489D-A7A4-9196E454DD3A bun exact.mjs ios`: succeeded and installed **Podcasts**, bundle ID `dev.bakeoff.podcasts.b18a827`, on the specified simulator. No other simulator was used.
- The bundled JSON is byte-for-byte identical to the source (420 unique episode IDs, 12 shows).
