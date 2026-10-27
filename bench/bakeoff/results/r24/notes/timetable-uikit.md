# Timetable build notes

Completed and installed as **Timetable** (`dev.bakeoff.timetable.beec609`) on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86`. The app is left running in standard light appearance. No other simulator was used.

## Implementation

- Programmatic UIKit app delegate, scene delegate, navigation controllers, and Departures / Saved tabs; no SwiftUI or storyboards.
- Full-day station schedules sorted by time with native pinned hour sections, local 24-hour times, coloured line badges, platforms, delay and cancellation messages, and dimmed past departures.
- Searchable station sheet, horizontally scrolling line filters, Now scrolling, visible save buttons, swipe actions, and departure details.
- Chosen station, saved departures, and the selected line persist through UserDefaults. Saved identifiers use the departure’s station, line, destination, scheduled minute, and platform.
- Dynamic colours, VoiceOver labels and actions, generous save targets, and vertical departure rows at accessibility text sizes. The empty Saved screen can scroll at the largest sizes. Compact header typography is bounded to preserve space for the schedule.
- All source transit data is shipped unchanged in `Resources/departures.json`. The actual supplied file contains **2,698** departures, five lines, and twenty stations, rather than approximately 6,000 departures. Nothing was synthesized or omitted.
- A custom departure-board app icon is included. `Tools/Icon.swift` reproduces it.

## Decisions and assumptions

The data has no service dates or timezone, so the schedule repeats daily in the device’s local timezone. Harbor, the first supplied station, is the initial selection. Times remain scheduled times; delay amounts are displayed separately. Now selects the first timetable entry strictly after the current minute, respecting the station and line filter. After the last departure, it shows the final departure and reports that no more departures remain today. Cancelled entries remain in chronological order. Saved is a collection across all stations, independent of the board’s current filter.

## What was hard

The most involved work was keeping self-sizing table rows aligned beneath pinned headers after a long Now jump, especially at the largest accessibility size. Font scaling and nested stack alignment needed simulator iterations. The final implementation uses fonts scaled once from standard-size baselines, vertically arranged accessibility rows, and a second scroll alignment using realized row and header geometry.

UIKit on iOS 26 integrates search into the bottom toolbar by default. The station picker explicitly keeps a stacked search bar below its navigation bar, with its native keyboard and dismissal controls.

No external web research or dependencies were needed. UIKit availability was checked by compiling against the installed SDK. Simulator commands required the execution environment’s elevated access to CoreSimulator.

## Verification

- Successfully built and installed using `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`.
- **171 simulator checks passed** in standard light mode and again in maximum accessibility text size with dark mode. Logs: `build/test-standard.log` and `build/test-large.log`.
- Checks cover every station and line, completeness and ordering, stable IDs, 00:00 and 23:59, strictly-after-time Now behavior, before/after-service scrolling, rendered Now targets and pinned-header alignment, search and selection, saving, layout, and state restoration.
- A separate terminate-and-relaunch check confirmed chosen station, selected line, and saved departure persistence, then restored the original preferences. Logs: `build/persistence-seed.log` and `build/persistence-check.log`.
- Visually inspected standard and largest-text screens, long station names, search with the keyboard, dark mode, Saved and its empty state, past/cancelled departures, and delayed departure details. Screenshots are in `build/`; `build/final.png` shows the installed app’s final launch state.
- Source and bundled JSON compare byte-for-byte; the generated Info.plist validates successfully.

The repeatable in-app checks run only with explicit `--self-test --exit-after-tests` launch arguments. `--preview <scenario>` opens visual scenarios with isolated preferences, and does not alter normal app state.

## Unfinished work and improvements to the toolkit

The final installed build also includes the last visual correction keeping the Now button at its natural width. No specified feature remains unfinished. A simulator touch/accessibility inspection driver would have made interactive verification faster; this toolkit provided simctl and in-app checks instead. A brief warning about iOS 26’s automatic bottom search placement and a supplied app-icon build step would also have saved iteration time.
