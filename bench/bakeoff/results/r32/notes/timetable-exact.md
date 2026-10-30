# Timetable build notes

Finished and installed as `Timetable` (`dev.bakeoff.timetable.bf10044`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86` with `bun exact.mjs ios`.

All requested features are implemented: the full-day station board, pinned hourly sections, searchable station selection, initial/Now positioning, line filtering, dimmed past departures, and persistent Saved departures and station selection. The original JSON is bundled at `assets/departures.json`; the data module also imports the supplied source JSON. All 2,698 supplied departures, twenty stations, and five lines are included.

## What was hard

- The supplied departures wrap through midnight rather than being in full-day chronological order. Sorting by minute with a stable original-row identity fixes both ordering and saved-item identity, including repeated times or destinations.
- Pinned headers come from Exact's native plain list. Explicit CSS sticky headers were redundant and caused browser overlap findings; removing them retained native pinning.
- Translucent fixed station controls allowed underlying departure text to cross their labels. The final strip uses the system background for legibility. Exact emits one stylistic `lower-list-beside-fixed` warning for that intentional choice. Native light/dark and largest-text layout checks report no findings.
- Persisted settings have a 16 KB limit. Numeric saved IDs keep even all 2,698 departures at 12,381 serialized bytes; string IDs would exceed the limit.
- The station button needed an explicit 44-point minimum touch target, found by native verification.

## Lookups and decisions

I read Exact's required start guide and looked up tabs, native list sections, scrolling, persistence, sheets, search, selectors, symbols, clocks, and simulator driving. Combined task syntax requires `when` before `key`.

The file contains scheduled minutes but no service dates, so the schedule repeats daily. Times use a consistent 24-hour local-time format. Harbor, the first supplied station, is the initial choice. Now selects the first scheduled minute strictly after the current local minute and shows an explicit end-of-day message after the last departure. Cancelled departures remain visible in their scheduled position. Saved covers all stations and is independent of the current board filter.

## Validation

- Six behavior tests pass on web and on the assigned iPhone simulator: day ordering and Now, searchable/persistent station selection, line filters, Saved across stations and relaunch, midnight/end-of-day behavior, and local time-zone offsets.
- Final `verify ios`: six passed, zero failed; no layout findings in light/dark; eight screens checked at accessibility extra-extra-extra-large (53-point body), with nothing spilling.
- Real-touch scrolling: nothing for the app to fix; zero measured hitches and no dropped lift frame. Exact declined to judge timing because the shared machine was busy. It separately reported its own status-bar scroll-to-top timing difference, which is outside this app's implementation.
- Final build and simulator installation succeeded. Only the assigned simulator was used.

No requested feature is unfinished. The remaining app-level diagnostic is the intentional opaque-control-strip style warning described above. The browser-only tour previously reported partially visible station/swipe labels at scroller edges at maximum text size; native verification found no such clipping.

## What would have made this faster

A transit-style starter with hourly sections and initial row positioning, a combined `when`/`key` example in the task overview, and clearer guidance reconciling fixed-control blur with overlap checking would help. The first compiler command also needed sandbox access to Exact's shared compiler lock. The tabs pattern, generated source types, deterministic clocks, native layout checks, and fast simulator plan updates saved substantial time.
