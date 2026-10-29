# Timetable

Built and installed as `dev.bakeoff.timetable.bc0dfd3` on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The final installed app was launched with `bun exact.mjs ios --sim CEDFB9C9-FF04-4617-B156-94C56B83CA86 --run`.

## Delivered

- Whole-day departures sorted by minute, with UIKit's pinned hour headers, colored line badges, platforms, delay labels, and cancelled destination strike-throughs.
- Searchable station picker, line filter, Now scrolling, past-departure dimming, and a Saved tab spanning all stations and lines.
- Chosen station and starred departures persist across launches. Native navigation, search keyboard, dark mode, Dynamic Type, and an app icon are included.
- The supplied file contains **2,698 departures**, five lines, and twenty stations. All records are included. An unchanged copy is bundled at `assets/data/departures.json`; its SHA-256 matches `data/departures.json`.

## Decisions and assumptions

- Times use a 24-hour display. The supplied timetable repeats daily in the device's local time zone.
- Ordering and dimming use the supplied scheduled minute; delay text is displayed verbatim.
- Now skips cancelled services. After the last service it shows the end-of-day message, with a View Full Day action, rather than wrapping into tomorrow.
- The line filter starts at All lines after relaunch. Saved departures remain independent of the board's station and filter.

## What was hard and what I looked up

- Used Exact's start guide and its tabs, persistence, list rows, clock, theme, asset, and iOS-driving references.
- Native list-button projection discarded badge backgrounds. Custom selectable rows fixed the line picker.
- Updating the station or line in the same commit that dismissed its picker left refreshed rows visible but not tappable. Deferring the selection by one commit fixed native interaction.
- Automatic scrolling needed to wait for both selection completion and data matching the selected station and line. Otherwise it could target a row from the previous filter.
- Large-text web sheets needed explicit field sizing and visible overflow on the sheet root while the list retained scrolling.

## Validation and remaining limitation

- `bun exact.mjs verify`: **clean**, ten tests passed, light/dark checks passed, and nothing spilled at the largest text size.
- `bun exact.mjs verify ios --sim CEDFB9C9-FF04-4617-B156-94C56B83CA86`: **ten tests passed**, no runtime refusals, and normal-size light/dark screens had no findings. Scrolling was reported as **like UIKit**.
- Validated every departure, sorting and grouping at all twenty stations, all 100 station/line combinations, and loading all departure IDs into Saved. The complete persisted ID list fits Exact's 16 KB setting limit.
- The native verifier still reports **21 overlap findings at maximum Dynamic Type** as row fragments pass beneath UIKit's translucent pinned section headers. Simulator screenshots and real-touch inspection showed ordinary pinned-header occlusion with wrapping row text. The report remains in `.exact/verify/ios-findings.txt`. Host load also limited timing judgments during scrolling measurements.
- No specification feature was omitted. The native verification report could not be made fully clean because of the pinned-header observations above.

## What would make this faster

Clearer documentation or diagnostics for native control projection, an explicit post-dismissal data-update hook, automatic suppression of transient scroll requests during selection changes, and verifier awareness of pinned supplementary-header occlusion would substantially shorten this workflow.
