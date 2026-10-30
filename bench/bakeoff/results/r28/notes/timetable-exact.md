# Timetable build notes

Finished and installed as **Timetable**, bundle **dev.bakeoff.timetable.ba69a40**, on simulator **C2188CC6-FBE9-409A-92A8-ABD89AC2C432**. The final command was `EXACT_SIM=C2188CC6-FBE9-409A-92A8-ABD89AC2C432 bun exact.mjs ios --run`; it installed and launched the bundled app.

## Completed

- Full-day station departures sorted numerically by minute, with native pinned hour headers.
- Searchable station sheet, native line filter, Now scrolling, past-departure dimming, coloured line badges, platforms, delays and cancellations.
- Saved departures across stations, persistent stars and chosen station, and useful empty and end-of-day states.
- Native navigation and tabs, keyboard handling, dark mode, Dynamic Type through the largest accessibility size, accessible control labels, and a custom transit app icon.
- The supplied file contains **2,698 departures**, **20 stations**, and **5 lines**, rather than approximately 6,000 departures. All records are included unchanged. `assets/data/departures.json` is a verbatim bundled copy of `data/departures.json`, and the data module imports that asset. The app works offline.

## What was hard

Large accessibility text needed a different row composition: the time, badge and star occupy one flexible row, and the destination and platform have the full width beneath it. Orange and yellow badges use dark ink for contrast.

The native list initially extended beneath the fixed station/filter strip. An opaque platform background and proper minimum touch heights resolved the control overlap. Start alignment for Now places the next departure below its pinned hour.

Replacing a station's entire native sectioned list could leave stale star hit targets. A returned data identity now keys each list instance, so new station and filter results get fresh native cells. I reproduced the issue with real touches, fixed it, verified the real-touch sequence, and added regression coverage.

## Lookups and decisions

Used Exact's tabs pattern and its documentation for plain sectioned lists, search, persistence, local time, tasks, scrollIntoView, themes/icons, bundling and simulator driving. No external service is needed.

The undated data is treated as a daily repeating schedule. Times use 24-hour local time. Delay and cancellation labels preserve the supplied scheduled order. Now selects the next strictly later scheduled entry; after the final entry it shows the end of the timetable and an explicit no-more-departures message. Saved entries remain useful on later days. Clock-based styling refreshes every 15 seconds; Now reads the clock immediately.

## Validation and remaining work

All specified features are finished. Six regression tests passed on web and iPhone, covering chronological ordering, strict next-departure timing, filtering, station search including no results, saving after station/filter changes, Saved across stations, relaunch persistence, removal, and midnight rollover.

`bun exact.mjs verify` and the final `bun exact.mjs verify ios` reported clean results. Light/dark mode and the largest text size were checked. A separate real-touch Saved check with two stations at the largest text size found no layout issues. Real-touch scrolling reported nothing for the app to fix; the shared host was busy, so the tool judged physics rather than overall timing.

The manifest documents intentional verification exceptions for partly visible scrolling rows passing beneath the required pinned hour headers, and station rows crossing a scroll boundary. Complete rows remain reachable and readable; these exceptions preserve standard native pinned-header behavior.

## What would have made this faster

A sectioned-list starter with station/filter replacement, explicit guidance about native list identity and fixed control strips, and verification that recognizes normal pinned-header occlusion would have reduced iteration. Read-only documentation commands that do not need access to the toolkit's build cache would also help.
