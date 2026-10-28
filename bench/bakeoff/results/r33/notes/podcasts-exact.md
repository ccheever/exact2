# Build notes

Podcasts is finished and installed on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96` as `dev.bakeoff.podcasts.bce1fb4`. The final installation command was `EXACT_SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 bun exact.mjs ios` and succeeded. No other simulator was used.

## Implementation decisions

- All 420 episodes from `data/episodes.json` ship through the data module's JSON import. The app works offline. Episodes are ordered by ISO date descending, then numeric ID for deterministic ties. Date-only values are formatted in UTC to avoid shifting the displayed day.
- The original played values remain part of the catalog. A persisted list of changed IDs records playback overrides; this supports changing both initially played and initially unplayed episodes. Up Next persists its ordered, unique episode IDs. These small lists fit Exact's persisted-setting limit comfortably.
- The three tabs each retain their own navigation stack. Episodes has live title/show search and a persisted unplayed filter. Shows displays counts that update immediately after playback changes. Detail supports Play/Pause, marking either state, and queue membership.
- Up Next supports real handle dragging, accessible Earlier/Later buttons, removal, and episode details. Playing marks the episode played immediately. Playing and pausing are visual states, as specified; no audio is implemented.
- The interface uses native navigation, tabs, search, switches, buttons, grouped lists, semantic colors, and text styles. At accessibility text sizes, episode actions move below the text and decorative show icons disappear so the content gets the full row width. A bundled waveform icon matches the app's purple accent.

## What was hard and what I looked up

- Exact's Contract language resembles TypeScript but disallows action loops and `reduce`. I used filtered predicates and `sum(map(...))` instead. A handler named `filter` also conflicts with the built-in array method and was renamed.
- I started from the tested tabs pattern and looked up list actions, persistence, bundled assets, native symbols, platform facts, and simulator driving in the local Exact documentation.
- Native grouped lists and reorderable virtualized lists have different requirements. Up Next uses the supported plain virtualized list with `reorderFor` and `reorderdrop`; the episode library uses native grouped rows.
- The largest text size initially exposed long-word overflow and narrow rows beside controls. It took multiple layout passes to preserve complete titles. The web tab labels also needed the supported capped Dynamic Type size appropriate for fixed-width tab cells.
- The simulator driver can run an updated plan without rebuilding the installed application. I explicitly ran the final `ios` command so a normal launch uses the finished embedded plan.

## Validation

- Seven web regression tests passed.
- Final iPhone verification: seven tests passed; first screen, seven test ends, and relaunch checked in light and dark mode with no layout findings.
- Largest iPhone Dynamic Type size, accessibility-extra-extra-extra-large (53-point body): nine screens checked, nothing spilled.
- Scrolling report: “like UIKit,” zero measured hitches, longest frame 16.7 ms, and zero dropped-frame lifts during 29.6 seconds of real touches.
- Additional real touch drives checked list marking, adding to Up Next, queue handle dragging, Earlier controls, the search keyboard, and opening a search result. The keyboard was visible while searching and dismissed on detail navigation.
- Persistence tests cover played/unplayed overrides, queue order and removal, and the unplayed filter across relaunch. Tests also cover original played values, duplicate-free queue membership, live show counts, newest-first order, search recovery, and Play/Pause.
- The compiler reports no app warnings. The native build emits an existing unused optional `wgpu-hal` patch warning but builds successfully.

## Toolkit limitation and unfinished work

All requested app features are complete. Exact's visual/functional verification reports “clean” on both web and iPhone. The separate semantic proof phase still makes the combined `verify` command exit 1: its oracle does not support `toLowerCase`, does not replay reload/back completely, and uses seeded source data rather than this imported catalog. It reports an at-boot search divergence, so no formal proof is claimed. The real web and iPhone test drivers pass against the shipped data.

## What would have made this faster

A small tested native queue pattern combining persistent order, drag handles, accessible move controls, and Dynamic Type adaptation would have helped. Clearer examples showing `exactViewport` as a root resource (rather than expression-like documentation), and separating unsupported semantic-oracle results from successful UI verification, would also reduce iteration time.
