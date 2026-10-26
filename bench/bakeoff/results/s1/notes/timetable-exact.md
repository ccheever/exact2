# Timetable build notes

Finished and installed as `dev.bakeoff.timetable.b87f681` / **Timetable** on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The final build was launched with `bun exact.mjs ios --run`.

All requested features are implemented: the full-day chronological board, native pinned hourly headings, searchable station selection, initial and button-driven Now positioning, line filtering, past-departure dimming, and persistent station and starred departures. Saved departures include their station and support direct removal.

## Data and decisions

The supplied file contains **2,698 departures**, rather than approximately 6,000. All twenty stations, five lines, and every supplied departure are included. `assets/departures.json` matches `data/departures.json` byte for byte and is shipped in the app; the data module imports it for offline use.

Times use a 24-hour clock. Now targets the next scheduled departure strictly after the device's current local time. After the final departure it shows the end-of-day message. Local midnight resets the interpretation of past and upcoming departures. This is the supplied daily timetable, without a live transit feed.

At accessibility text sizes, the station selector moves into the native navigation bar and the count summary is omitted to leave more space for departures. Row text wraps and rows grow; no fixed text sizes or line clamps were used.

## What was hard and what I looked up

- Exact's root component must appear before child components. An action named `filter` also shadows the array method and needed renaming.
- Native list cells needed to be rebuilt when changing station or line. Keying the list by those values fixed star-button interaction after the station sheet closed.
- UIKit self-sizing cells needed a second scroll alignment after mounting. Two short task turns make Now accurate at the largest Dynamic Type size.
- I looked up Exact's sections, native menus, search, persistence, asset packaging, scrolling, timer tasks, and iOS driving. UIKit already pins plain-list section headers; extra authored sticky styling was unnecessary.
- The largest-text checker reported portions of scrolling cells intersecting UIKit's pinned headings. Simulator inspection confirmed standard clipping/blur beneath those headings and that full row content scrolls clear. These intentional intersections are documented in `app.json` under `verify.accept`.

## Validation

- Nine functional tests pass on iPhone, covering midnight ordering, cancelled/delayed statuses, station search and persistence, line filtering, Now, cross-station saving, relaunch persistence, removal, and empty and populated Saved screens.
- Final `bun exact.mjs verify ios`: **clean**, including light/dark checks and eleven screens at AX5, with the documented pinned-header acceptances.
- Real-touch scrolling found nothing for the app to fix: zero measured hitches and no dropped lift frames. Heavy machine load prevented a comparative timing judgment. Exact separately reports a framework-level status-bar scroll-to-top timing difference.
- The web host's nine functional tests also passed; the iPhone was the delivery and final layout-validation target.
- Bundled data was checked for valid station/line references and minute ranges, and compared byte for byte with the source.

No requested app features remain unfinished.

## What would have made this faster

A sectioned-list starter with scroll-to-current-item behavior, guidance on native cell identity when replacing a board, an example of scrolling after self-sizing, and a preflight explaining shared compiler-cache write permissions would have saved time. Initial native compilation took several minutes; later plan updates were fast. The largest-text checker could better distinguish intentional native pinned-header overlap from actual row-layout collisions.
