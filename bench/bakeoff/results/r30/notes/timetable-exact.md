# Timetable implementation notes

Finished and installed as **Timetable**, bundle ID `dev.bakeoff.timetable.bd0113a`, on simulator `C2188CC6-FBE9-409A-92A8-ABD89AC2C432`.

## Implemented

- The complete station day, sorted by scheduled minute, with native pinned hourly sections.
- Time, destination, colored line badge, platform, and supplied delay/cancellation status in every departure row.
- A native searchable station sheet containing all twenty stations.
- A line picker with all five lines and All lines; the navigation subtitle shows the active filter.
- Automatic positioning at the next departure on opening, station changes, and filter changes; Now repeats the jump.
- Past departures dim as local time advances, including correct rollover at midnight and an end-of-service message.
- Star/unstar controls and a Saved tab covering all stations, plus swipe removal. Saved IDs and the chosen station persist across launches.
- Offline bundled data, native navigation and tabs, dark mode, Dynamic Type, selection haptics, and a custom app icon.

## Data and decisions

The supplied JSON contains **2,698** departures, five lines, and twenty stations, rather than the specification's approximate 6,000. It is bundled intact through the data module's JSON import. No departures were invented or discarded. All station/line references and departure minutes were validated.

Times use a consistent 24-hour local clock. Since departures have no date, the timetable and Saved entries repeat daily. Delay text is displayed as supplied; chronological ordering uses the supplied scheduled minute. Now selects the first listed departure strictly after the current local time. After the last departure, it goes to the end-of-service message.

Stable numeric IDs come from each record's original position in the bundled file. Saving every record would use 12,381 bytes, within Exact's 16 KB primitive persistence limit.

## What was hard / looked up

I read Exact's start guide and looked up native plain lists, section headers, navigation bars, modal search, persistence, clocks, scrollIntoView, symbols, icon tokens, and simulator-driving commands. The tabs pattern supplied the starting structure.

The main layout challenge was keeping the station and line controls clear of large departure text. A separate wrapping control strip caused overlap findings at the largest text size. Moving these controls into UIKit's navigation bar resolved that and preserved room for pinned hour headings.

Automatic scrolling needed to wait for loaded data and a mounted, visible departure route. A short scheduled action and a route guard prevent scrolling toward an inactive tab. The next-departure calculation reads the current clock directly instead of relying on the periodic display refresh.

Orange status text and white text on the supplied orange line color failed contrast checks. Delays now use the semantic secondary label, and orange/yellow badges use dark ink.

The restricted filesystem initially prevented Exact from opening its shared compiler lock. Exact's own build/documentation commands then ran with permission to access their shared toolkit cache.

## Validation

- Final `bun exact.mjs verify ios`: **clean**.
- **7 passed, 0 failed** behavior tests: complete day/order/status, searchable station selection and persistence, line filtering, Saved persistence/removal across stations, midnight/end-of-service handling, and both picker screens.
- All checked iPhone screens pass light and dark checks.
- At accessibility-extra-extra-extra-large (53 pt body), all nine checked iPhone screens fit without spills.
- Native touch scrolling: no app-controlled issues, 0 ms/s hitches, longest frame 16.7 ms, no dropped lift frame.
- Direct simulator checks also covered the longest station name and the station picker at maximum text size.
- Final `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios` built and installed successfully.

## Remaining limitations

No requested iPhone feature is unfinished. Exact reports its own slightly slower status-bar scroll-to-top animation; the report explicitly identifies that as toolkit behavior outside the app's control.

The optional web verifier passes all seven behavior tests and regular-size layout/contrast checks, but flags eight maximum-size clipping cases involving partially visible sheet rows or swipe-revealed content. The native iPhone verification is clean.

## What would have made this faster

A dedicated sectioned departure-list pattern, a documented example of initial scrolling into a native plain list, and clearer guidance for placing persistent controls around Dynamic Type lists would have saved iteration. Web checks that distinguish normal scroll-port clipping and swipe animation from actual text truncation would also reduce noise. The native pinned headers, tab pattern, fast plan updates, and consolidated iPhone verification were useful.
