# Timetable build notes

- Built with Exact using its tabs pattern, native iOS navigation/search, a native plain sectioned list with pinned hour headers, a native line picker, and persistent state.
- The supplied file contains 2,698 departures, five lines, and twenty stations (rather than approximately 6,000 departures). Every supplied record is included. `assets/departures.json` is an unchanged bundled copy of `data/departures.json`; the data module imports it directly, so the app works offline.
- Chose Harbor as the initial station and 24-hour times for a compact, unambiguous daily timetable. Saved departures cover all stations and lines, independent of the current board filter. After the last departure, Now keeps today's final services visible and explains that there are no more today.
- The input is not chronological: overnight departures appear late in the file. The data module sorts by minute before grouping and uses stable source-index identifiers for persistence. Compact identifiers keep even all 2,698 saved departures within Exact's persisted-state size limit (14,857 bytes).
- Looked up Exact's native list sections, scroll commands, search, time-zone clock, persistence, theme tokens, reusable components, and iOS driving/verification commands.
- Whole-row opacity made past departures too faint. Past times and destinations instead use dimmer colors with readable contrast; route badges and disruption warnings retain full contrast.
- A native section header can cover a top-aligned scroll target. Now gives its target an inset so the heading covers spacing and the full departure stays visible. It aligns again after UIKit measures an offscreen cell; this matters when jumping across midnight. A clipping container on the native host keeps scrolled rows beneath the fixed controls. Native cells are recreated when entering or leaving accessibility text sizes to refresh their measurements.
- Adding static assets while the initial native prebuild was running invalidated the bake receipt. The next build correctly rebuilt. Native Swift compilation was the slowest part of the build loop.
- Faster iteration would come from read-only documentation commands that do not need a writable shared Cargo lock, a prebuild that tolerates added assets, and section-header-aware top alignment for native scroll targets.

- The largest-text tour initially reported partial rows under pinned headers. Inspected these with screenshots and refreshed cell identities on text-size changes. The final full iPhone run had no findings and accepted zero exceptions; no verification exceptions remain configured.

## Completion and validation

All six requested features are implemented. No requested features remain unfinished.

- `bun exact.mjs verify`: eight tests passed; light/dark, both widths, relaunch, and largest-text checks clean.
- `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs verify ios`: eight tests passed; normal and largest text, light/dark, relaunch, and native scrolling physics clean. Zero findings were accepted. The final manifest contains no verification exceptions.
- Checked a populated Saved list separately at the largest text size in both color schemes, with no findings. Real touch opened the native line menu; the driver required its select-value operation to choose an option.
- The scrolling audit found nothing for the app to fix. Host load prevented judging performance timing, although the recorded longest frame was 16.7 ms. Exact reported its own status-bar scroll-to-top timing difference.
- Finished with `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios --run`. Timetable (`dev.bakeoff.timetable.b6bbfcf`) is built, installed, and launched on that simulator. Restored a clean initial station/Saved state before the final launch.
