# Timetable build notes

Finished and installed as `dev.bakeoff.timetable.baa7935` on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. The final production build is launched. No other simulator was used.

## Implementation

Programmatic UIKit app and scene delegates, native navigation and tab bars, plain UITableView departure boards with pinned hour headers, searchable station sheet, native single-selection line menu, and independent star buttons. The Saved board includes originating stations and supports unstarring. UserDefaults persists the station and stable departure identifiers.

The complete supplied JSON is shipped unchanged in the bundle. It contains 2,698 departures, 20 stations, and five lines; the approximate 6,000 in the brief differs from the supplied file. No records were fabricated or omitted.

## What was hard

- Scrolling to Now correctly after automatic row measurement, while retaining the pinned heading for that hour.
- Supporting the largest accessibility text size without letting the station header consume the whole screen. Departure rows change to a vertical layout; secondary header typography is capped and decoration is reduced.
- Coordinating search, the keyboard, and dismissal of the containing station sheet. The final check waits for the native dismissal animation to complete.
- Simulator access required running the build and simctl commands outside the filesystem sandbox because CoreSimulator is a system service.

## Assumptions and lookups

No web research or third-party dependencies were needed. UIKit API availability was checked by compiling and running with the installed SDK.

The JSON is a repeating daily schedule in the device's local time, displayed using 24-hour times. Sorting, past dimming, and Now use the supplied scheduled minute; the provided delay remains an explicit status rather than moving a departure to a different time. Now selects a strictly later scheduled minute, including cancelled entries since the board lists every scheduled departure. After the final departure, Now scrolls to an explicit end-of-day message. The board refreshes as the clock changes, at foreground entry, and on significant time changes.

## Verification

Used only the assigned simulator. Passed the full 16-scenario simulator check sequence plus the final focused search-selection check. Coverage included:

- Bundled data, unique stable IDs, all 20 stations, complete daily listings, ordering, and every station/line filter combination.
- Before-first, midnight, current-minute, and after-last departure behavior.
- Actual star-control actions, Saved contents, removal, and preferences restored after process termination and relaunch.
- Station searching, no-results state, selection with an active keyboard, and sheet dismissal.
- Pinned-header geometry and returning to upcoming departures with Now.
- Light and dark mode, the largest accessibility text size, empty Saved, readable rows, and minimum 44-point star targets.

Screenshots and logs are in `build/checks/`; `build/final.png` shows the installed production build. The final bundle's ID, name, scene manifest, app icon, and byte-for-byte data copy were also verified. Test-created saved entries were removed and the initial station restored to Harbor.

`Checks/AppChecks.swift` is an opt-in integration harness. It and the test-only clock override are excluded from the production build. To reproduce the checks, first run the normal build, then compile `Sources/*.swift Checks/AppChecks.swift` with `-D DEBUG` using the same simulator target as build.sh, sign the bundle, and run `python3 Checks/run_checks.py`. Finish with the normal build command below to restore the production binary.

## Remaining work and what would help

No known unfinished specification features. A dedicated simulator UI-driving tool or ready-made UI test target would make gesture-based verification faster. Explicit scheduled-versus-delay-adjusted time semantics and the exact expected record count would remove the small data ambiguities.

Rebuild and launch:

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```
