# Timetable build notes

Finished as a programmatic UIKit app, bundle ID `dev.bakeoff.timetable.ba2863b`. The final build is installed and launched on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. No other simulator was used. The temporary UI test runner was removed from the simulator afterward.

## What ships

- The complete original JSON, copied byte-for-byte into the app bundle: five lines, twenty stations, and **2,698 departures**. The supplied file contains fewer departures than the approximate 6,000 mentioned in the specification; none were omitted or invented.
- The whole-day board, numeric time ordering, pinned hour headings, station search, single-line filtering, Now scrolling, past-departure dimming, delay/cancellation labels, and persistent station and saved departures.
- A Saved tab that includes each departure’s originating station, an empty state, dark appearance, Dynamic Type, VoiceOver labels, haptics, and a custom transit icon.

## Decisions and assumptions

The undated dataset is a recurring daily schedule in the device’s local time zone. Times remain the scheduled times, including for delayed services; the exact supplied delay or cancellation status is displayed separately. Now uses the first scheduled departure strictly after the current local time, within the active station and filter. If no later departure exists, it scrolls to the end and explains that today’s service is finished. A departure becomes past at its scheduled minute. The board updates as time advances and resets its day at midnight. Saved identities use the station, line, destination, minute, and platform rather than row position.

## What was hard / what I looked up

Self-sizing rows, pinned headers, and initial Now scrolling needed simulator checks together. The fixed header required a separate accessibility-size arrangement: a native line menu and Now replace the horizontal filter strip, and departure details stack vertically. Long station names and Saved metadata remain readable. Yellow and orange badges use dark lettering for contrast.

Dismissing a searchable sheet needed to dismiss its navigation controller so the search presentation could not remain over the board. I also replaced the initial search-empty presentation with a table header that stays visible above the keyboard. The simulator’s newer search UI exposes a Close control rather than a text Cancel button; UI automation also retains some offscreen cells, so the empty-result assertion checks visible, hittable rows.

I inspected the local simulator command help and Xcode’s bundled XCTest framework/runner layout to build an automated UI harness without an Xcode project. No web research or external packages were needed.

## Verification

The final standard-size suite passed three cases: full board interactions and persistence, empty search and clearing/dismissal, and dataset/time-boundary assertions. The final dark-mode, maximum-accessibility-text-size interaction case also passed. Checks cover all five filter counts, all twenty stations’ sorted data, unique saved identities, exact departure-minute boundaries, midnight, the last minute of the day, empty input, and local wall-clock time around a daylight-saving date. Screenshots were reviewed in light, dark, and maximum text size; they are in `build/screenshots/`. Final test results are in `build/FinalFunctionalTests.xcresult` and `build/FinalAccessibilityTests.xcresult`.

Rebuild and launch with:

```sh
SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run
```

## Unfinished work / what would have helped

No requested feature remains unfinished. A ready-to-run simulator interaction harness would have saved the most setup time. Explicit rules for recurring days, delayed-time ordering, and behavior after the last departure would have removed the small product assumptions above.
