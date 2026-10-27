# Timetable build notes

Implemented the complete specification in programmatic UIKit. The installed app is `dev.bakeoff.timetable.b45416d`, named Timetable. Only simulator `498D6D1F-C21C-4B31-90F4-5D857216E256` was used.

## Data and decisions

- The supplied file actually contains 2,698 departures, five lines, and twenty stations. It is shipped unchanged as `Resources/departures.json` and copied into the app bundle by `build.sh`.
- The bundled schedule represents a recurring day. The board uses the device's local clock and a 24-hour time display. Now selects the first scheduled minute strictly after the current minute, respecting station and line filters. After the final departure it shows the last trip and an explicit end-of-day message; it never silently wraps to tomorrow.
- Harbor is the initial station. The chosen station and starred departures persist in UserDefaults. Saved covers every station, shows origins, and supports the same line filters.
- Departure identity includes the complete source record, so distinct departures at the same time remain distinct.

## What was hard

The main challenge was maintaining a useful board at the largest accessibility text size. The standard layout initially devoted too much space to the header. At accessibility sizes, decorative captions are hidden, Now moves to the navigation bar, and departure rows become vertical with wrapping destinations and status text. Route badges and filter controls retain manageable dimensions. Native plain-table section headers provide pinned hours.

Other care points were keeping scroll position when starring, updating Saved immediately, preserving all earlier trips when jumping to Now, handling the last service of the day, and refreshing past-trip styling when the clock or day changes.

## Research and verification

I used the supplied README, local compiler feedback, and simulator screenshots. No external packages, web research, or external services were required. Some UI choices were inferred: default station, 24-hour formatting, recurring daily interpretation, and showing all saved stations together.

The app passed 266 opt-in simulator checks covering bundled data, ordering, all station/line combinations, complete hour grouping, midnight/end-of-day targeting, dimming, status labels, search (including whitespace/case/no results), starring controls, Saved updates and empty state, invalid preferences, and persistence. A separate process-termination/relaunch check confirmed that station and saved departures survive a real relaunch.

Visual reviews included light and dark appearances, the maximum accessibility text size, pinned hour headings, cancellation rows, populated Saved, station selection, and station search with the software keyboard. Screenshots are in `build/`.

To rerun the in-app checks after building:

```sh
xcrun simctl launch --terminate-running-process --console 498D6D1F-C21C-4B31-90F4-5D857216E256 dev.bakeoff.timetable.b45416d --self-test --preview --exit-after-tests
```

The `--preview` checks use an isolated preference suite and do not change normal user preferences.

## Unfinished work

None of the required features are unfinished.

## What would have made this faster

A supplied UI automation runner with simulator touch/typing support would have reduced the need for opt-in in-app UI checks. An exact data schema/count and explicit guidance on daily repetition, time format, and end-of-day behavior would have removed the small interpretation decisions above.

## Build

```sh
SIM=498D6D1F-C21C-4B31-90F4-5D857216E256 ./build.sh --run
```
