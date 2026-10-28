# Timetable build notes

Finished and installed as **Timetable** (`dev.bakeoff.timetable.bbdb667`) on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`. The final command was `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios --run`.

## What ships

- The entire supplied data set: five lines, twenty stations, and **2,698** departures. The specification's approximate 6,000 count differs from the actual file; no departures were invented or discarded.
- `data/departures.json` is imported into the bundled data module. An unchanged raw copy also ships as `assets/departures.json`.
- A full-day, sorted departure board with native pinned hour headers, colored line badges, platform and disruption details, and muted past times/destinations.
- Native searchable station selection, a line menu, Now positioning, and a Saved tab. Saved departures use an atomic local JSON file; the selected station uses persisted state.

## What was hard

- The source data appends after-midnight departures after late-night departures. Sorting by minute before grouping was essential to show the whole calendar day correctly.
- A fixed filter strip conflicted with scrolled content at large text sizes. Moving the filter into a native navigation-bar menu resolved it.
- UIKit already pins plain-list section headers. Removing duplicate sticky styling and using start alignment for Now produced the correct native behavior.
- Dimming a complete row reduced badge and status contrast too far. Muted light/dark text colors distinguish past departures while keeping line and disruption information readable.
- A navigation-bar button containing text and an icon displayed only its icon. A text-only station button keeps the chosen station visible.
- Adding assets during the initial background build invalidated its bake receipt. Subsequent builds reused the cache.

## Lookups and decisions

Read Exact's start guide and used the tabs pattern. Looked up native list sections, modal search, persistence/files, clocks, pull-down menus, theme tokens, scrolling, iOS driving, and verification.

The file describes a daily schedule rather than dated service. Saved departures therefore refer to stable records in that repeating daily schedule. Now means strictly after the current local time; after the last departure it shows the timetable's explicit end-of-day message. Cancelled services remain listed and may be saved.

## Validation

- Eight behavioral tests passed on both web and the assigned iOS simulator. Coverage includes ordering and statuses, case-insensitive search and no results, station persistence, line selection, saving/removing across relaunch, cross-station saved ordering, end of day, and midnight.
- Final `verify ios`: all eight tests passed; light/dark screens were inspected; **nothing spills across ten screens at the largest accessibility text size**.
- Real-touch scrolling: no app-fixable findings; zero measured hitches, 16.7 ms longest frame, and normal native deceleration. Exact declined to judge machine-dependent timing because the host was busy.
- Relaunch verification recovered the selected station and saved records.
- Native screenshots and reports are in `.exact/verify/`.

## Remaining limitations

No requested app feature is unfinished. Exact's overall verifier does not return a clean exit: it reports seven covered elements on rows partially scrolled underneath native pinned headers in two test-end screenshots. These are ordinary scroll-edge clipping, reviewed visually; the next departure itself opens fully visible. No broad verifier suppressions were added.

The additional synthetic semantics checker also reports unsupported `toLowerCase` operations, cannot replay reload synchronously, and reports a divergence involving generated swipe-delete identifiers for synthetic nonnumeric departure IDs. Actual bundled-data behavioral tests pass. These toolkit reports are retained rather than described as successful formal verification.

## What would have made this faster

A schedule-oriented starter combining native hour sections, a navigation filter menu, and initial row positioning; clearer examples of navigation-bar button text/icon behavior; and verification that recognizes native pinned-header clipping. Preparing bundled assets before starting the background native build would also avoid one rebuild.
