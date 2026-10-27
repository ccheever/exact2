# Timetable build notes

Finished and installed as **Timetable** (`dev.bakeoff.timetable.b43998c`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final `bun exact.mjs ios --run --sim …` completed and launched the standalone app.

## Implemented

- Complete station timetable, sorted by minute and grouped into native pinned hour sections.
- Searchable picker for all twenty stations; station selection persists.
- Initial positioning and Now jump to the next scheduled departure strictly after the current local time. After the last departure, Now reaches the end-of-day message.
- All-lines or single-line filtering; times, original line colors, destinations, platforms, delays and cancellations.
- Past departures use dimmer time and destination text while retaining readable badges and active save controls.
- Saved departures across stations, ordered by time, with atomic file persistence and queued star changes.
- Native navigation, search, menus, tab bar, haptics, dark mode and Dynamic Type wrapping.

## Data and assumptions

The supplied JSON actually contains **2,698 departures**, five lines and twenty stations. It remains unchanged and is imported into the bundled data module in full. Records following midnight appear at the ends of station blocks in the source; sorting puts them at the start of the displayed day.

Harbor, the first supplied station, is the initial selection. Times use 24-hour notation and the device's local clock. This is a daily static timetable: saved entries identify schedule records and remain saved on subsequent days. Filtering and Now use the supplied scheduled minute, including for delayed or cancelled records.

## Validation

- `bun exact.mjs verify`: nine tests passed; clean report, including largest-text layouts.
- `bun exact.mjs verify ios --sim EF198926-1958-489D-A7A4-9196E454DD3A`: nine tests passed; no unaccepted light/dark layout findings; nothing spills at accessibility-extra-extra-extra-large (53 pt body).
- Simulator drives covered station search with the keyboard, Ferry Building's longer station name, real maximum accessibility text, Now and scrolling.
- Tests cover sorting, grouping, filtering, station persistence, save/un-save and relaunch, midnight, end of day, station search and empty states.
- Intentional native pinned-header overlays have explicit verification exceptions in app.json. Earlier rows remain accessible by scrolling upward.

The last scrolling benchmark reported one isolated **three-frame touch-to-content lag**, despite two earlier passes reporting nothing for the app to fix. The final measurement otherwise had zero hitch ratio, a 16.7 ms longest frame, 4.15 ms main-thread p95, no dropped lift frames, no blank samples and normal deceleration. The app has no per-frame scroll handler. Exact also reported its own status-bar scroll-to-top timing difference. These toolkit/measurement findings remain recorded; no requested app feature is unfinished.

## What was hard / what I looked up

- Exact's child-component props, task-clause order, persistence limits, native search, source imports and scrolling commands needed targeted vocabulary lookups.
- Native plain sections already pin their headers. Confirming this in the toolkit avoided an unnecessary custom scrolling implementation.
- A navigation-bar button containing both a station name and a symbol displayed only the symbol. A text-only native bar button keeps the selected station visible.
- White text on the supplied yellow and orange fills failed contrast. Those badges now use black ink; other badges use white ink, declared as intentional fixed theme tokens.
- Separating the fixed filter strip from scrolling content needed an opaque native background. Large-text checks exposed this boundary issue.
- The initial prebuild captured the old scaffold identifier in app.ts after the manifest had been renamed; matching both identifiers resolved it.

## What would make this faster

A tested timetable pattern combining native pinned sections, a fixed filter strip, Now positioning and large-text checks would help. Clearer documentation of bar-button text-plus-image behavior and fewer false-positive overlap findings around pinned headers would also reduce iteration time.
