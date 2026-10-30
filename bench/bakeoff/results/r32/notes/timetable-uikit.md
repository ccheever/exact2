# Timetable build notes

Implemented a programmatic UIKit app with native navigation, a searchable station sheet, native line menus, pinned hour sections, a Now jump, past-service dimming, status labels, save buttons and swipe actions, and a persistent Saved tab. The original JSON is shipped unchanged in `Resources/departures.json`. The actual source contains **2,698 departures**, not approximately 6,000.

## Decisions

- The bundled timetable repeats daily in the device’s local time zone. Times use 24-hour notation so the whole-day board is unambiguous.
- Now selects the first scheduled minute strictly after the current time, respecting the chosen station and line. After the final service, it shows the end of the board with an explicit message. Cancelled services remain on the board and delay labels do not alter scheduled sorting.
- Saved spans all stations and includes the origin station in each row. Its line filter is independent of the departure board’s filter.
- The source has no departure IDs. Persistence uses the original array offset plus service fields, distinguishing even duplicate-looking departures.
- Accessibility text sizes use a simpler station header, stacked controls, vertical departure rows, and hour headings without the visual count. VoiceOver still reads the count and all service details. Yellow and orange badges use dark lettering for contrast.

## What was hard

UIKit’s reusable, self-sizing stack layouts needed careful time-column sizing; multiline intrinsic widths initially squeezed destinations out. The largest accessibility size also required an actual alternate layout rather than simply enlarged fonts. Searching inside a presented navigation controller required dismissal from the presenting controller so choosing a station closes the entire sheet. The runtime’s standard content-unavailable search view was visually correct but not exposed reliably to UI automation; a native-label empty state fixed accessibility and testing.

## Lookups and assumptions

Used local `simctl` help for appearance/content-size controls and Xcode’s UI-test runner. No web services or third-party dependencies were needed. The daily schedule, local time zone, strict interpretation of “after,” and cross-station Saved behavior were inferred from the specification.

## Verification

All builds and tests use only simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`.

- Full UI journey passed in dark mode at the largest accessibility text size: station search and selection, native line filtering, Now, saving, relaunch persistence, and empty search results (`build/ui-tests6.log`).
- Final standard-size UI journey passed, including removing the final saved departure and restoring Harbor (`build/ui-tests-final.log`).
- All **263 in-app checks passed**: all stations and line filters, full-day ordering and grouping, unique IDs, persistence, search normalization/empty results, dimming time comparisons, and Now boundaries including the end of service (`build/self-test.json`).
- Confirmed the bundled JSON exactly matches the supplied file.

The UI test project is in `Tests/`. In-app data checks run with the explicit `--self-test` launch argument and write `Documents/self-test.json`. The standard build command remains:

```sh
SIM=486FD498-0E3E-4B64-8E4B-152BD8265EE7 ./build.sh --run
```

## Unfinished work and what would help

No known unfinished specification features. A starter UI-test target, stable departure IDs in the data, and an accurate source-record count would have made this faster. No external account, network access, or runtime downloads are required.
