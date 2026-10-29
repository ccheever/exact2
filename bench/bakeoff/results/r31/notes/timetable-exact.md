# Timetable — build notes

Finished and installed on the requested simulator, `498D6D1F-C21C-4B31-90F4-5D857216E256`, using `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs ios`.

- App name: Timetable
- Bundle ID: dev.bakeoff.timetable.bf65f68
- Built with Exact, using native iOS navigation, tabs, lists, pinned section headers, search, radio selection, and line selection.
- The supplied dataset actually contains 2,698 departures, twenty stations, and five lines. All entries are included. The data module imports the original JSON; `assets/departures.json` also ships a byte-for-byte copy of the original file.

## Behavior and decisions

The whole-day board sorts by minute after midnight before forming hour sections. This matters because the supplied file puts its midnight departures after its evening departures. The first station, Harbor, is the initial selection. Times use unambiguous 24-hour notation. Now uses local device time, follows the active line filter, and also runs when the board first opens or its station/filter changes. After the last departure it scrolls to the final departure and displays an end-of-day message.

Earlier departures are dimmed; scheduled times remain unchanged for delayed or cancelled services. The supplied statuses are displayed verbatim. The timetable repeats each day because the supplied dataset contains no dates. Saved departures retain their original station and appear in time order across all stations. Favorites use a durable JSON file with atomic writes and queued mutations; the selected station uses persisted settings.

## Verification

- `bun exact.mjs verify`: nine behavior tests passed; normal-size light/dark checks passed. Its web-only largest-text pass reports some labels partially crossing a scrolling sheet's viewport boundary. The iPhone-specific largest-text pass below reports no spills.
- `EXACT_SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 bun exact.mjs verify ios`: nine tests passed; first screen, nine test ends, and relaunch checked in light and dark with no layout or contrast findings. Eleven screens at the largest accessibility text size had nothing spill.
- Cold-launch verification retained both the chosen station and a saved cancelled departure from another station.
- Real-touch native scrolling: no app-level issues, 0 ms/s hitches, 16.7 ms longest frame. Exact separately reports its own status-bar scroll-to-top timing difference.
- Additional simulator drive confirmed that 19:00 UTC in America/Los_Angeles selects the 12:09 local departure, that filtering to Green selects 12:14, and that real touches can star it and open Saved. Light/dark checks passed.
- Dataset station/line references, minute ranges, and bundled-copy equality were checked.
- Final `bun exact.mjs ios` build and installation succeeded.

Screenshots and verification output are under `.exact/verify-ios/`, `.exact/verify/`, and `.exact/final-touch.png`.

## What was hard / looked up

I read the required Exact start guide and consulted the list, scrolling, persistence, search, clock/date, file-grant, native-control, icon, and iOS-driving documentation. File storage must live under `app:/data/`; an initially shorter path compiled but failed at runtime, and was corrected. Native radio changes deliver their string value. Initial scrolling needs to run after the data has mounted; `after(1, ...)` also makes this deterministic under Exact's frozen test clock.

The main layout challenge was keeping the station and filter controls clear of list content at the largest text size. The finished bar uses the system's opaque background, and the list stops below it. Exact retains one style warning, `lower-list-beside-fixed`, because it prefers a translucent scroll-under treatment. The opaque boundary is intentional: the iPhone verification confirms no overlapping controls or clipped text. No requested feature remains unfinished.

Faster iteration would benefit from a documented fixed filter-bar pattern that works with pinned hour headers and Dynamic Type, clearer file-path validation at compile time, and fewer web checker reports for text crossing a scroller's viewport boundary. Native build caching, typed source interfaces, and the integrated iOS verification otherwise made the work straightforward.
