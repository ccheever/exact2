# Timetable build notes

Finished and installed as **Timetable**, bundle ID `dev.bakeoff.timetable.bcb4e10`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used. The final command was `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs ios --run`.

## What ships

- The complete supplied `data/departures.json` is imported into the offline bundle. It actually contains **2,698 departures**, five lines, and twenty stations; no records were invented to reach the specification's approximate count.
- The departure board sorts the complete station day by scheduled minute, including departures after midnight that occur late in the original file. Native plain-list sections provide pinned hour headings.
- Station search is live, case insensitive, and trims surrounding whitespace. The selected station persists. A native line selector filters one line or all lines.
- The board opens at the next scheduled departure strictly after the current local time. Now centers that departure; after the final departure it scrolls to the end-of-day message. Earlier departures remain available and have muted, accessible text.
- Stars persist and appear in a Saved tab independently of the current station or line filter. Numeric saved identifiers let every supplied departure be saved within Exact's 16 KiB persisted-state limit: all identifiers together occupy 12,381 JSON bytes.
- Native navigation, station sheet, keyboard/search integration, tab bar, haptics, and a custom app icon are included.

## Validation

`bun exact.mjs verify` ran the five functional tests successfully. Its web-only intermediate modal scan reported partially scrolled station labels at the viewport edge as clipped at maximum text size.

The final `EXACT_SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 bun exact.mjs verify ios` was **clean**:

- Five tests passed: day ordering/statuses, station search/persistence, line filtering, saved departures/persistence across station and filter changes, and local midnight/end-of-day behavior.
- Initial screen, test ends, and relaunch: no light/dark layout or contrast findings.
- Largest Dynamic Type size, body 53 pt: nothing spills on all seven checked screens.
- Real-touch list scrolling: nothing for the app to fix; 0 ms/s hitches, longest frame 16.7 ms, main thread p95 3.73 ms, no dropped frame on the measured lift.

Additional native checks covered the station picker and no-results state with the keyboard and largest text size, in both appearances. They had no findings. Screenshots and reports are under `.exact/` and `.exact/verify/`.

Exact's optional differential semantic checker does not support `toLowerCase` and its synchronous oracle does not replay relaunches. These are checker limitations; the real iPhone tests exercise search and persistence and pass. Exact also reports its own status-bar scroll-to-top animation as slower than UIKit's reference; its report explicitly says this is not app-controlled.

## What was hard / what I looked up

- Read the required Exact start guide and started from the tested tabs pattern. Looked up sectioned lists, native search, persisted settings, timers/local time, scroll commands, selects, reusable component props, symbol roles, and app assets.
- Native scroll-edge behavior initially let prior rows show beneath the fixed control strip. An explicit strip background, a minimum 44-point station target, and centering the Now destination fixed the native findings.
- Whole-row opacity compounded with scroll-edge fading and reduced contrast. Past text now uses readable light/dark muted ink while line badges retain their supplied colors. Yellow and orange badges use dark lettering for contrast.
- The entire persistence set needed a size check; string identifiers could exceed the setting limit, so saved identifiers are numeric.
- Toolkit commands need access to Exact's shared build cache. Initial restricted access failed; approved tool executions resolved this.
- Driver targets containing spaces need quotes inside the operation string. Separate driver invocations preserve persisted data but start navigation afresh.

## Decisions and unfinished work

All requested features are complete. Times use a 24-hour clock and the device's local time zone. Since the source has minutes after midnight and no service dates, it is treated as today's schedule; saved departures refer to stable entries in that bundled schedule. Status text is shown exactly as supplied. Cancelled departures remain visible and can be starred.

## What would make this faster

A transit-board starter combining pinned hour sections, a fixed native filter strip, and automatic initial scrolling would help. Documentation that makes fixed-control scroll-edge behavior and persisted-state size limits prominent would avoid iteration. The verifier could distinguish offscreen modal rows and native pinned-header occlusion from real clipping more reliably, and its semantic oracle could support ordinary case-insensitive search and relaunches.
