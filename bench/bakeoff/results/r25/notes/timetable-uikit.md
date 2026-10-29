# Timetable — build notes

Completed and installed as `dev.bakeoff.timetable.b3882d3` on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`. The app uses programmatic UIKit with an app delegate, scene delegate, native navigation, tab bars, tables, search, and line-selection menus. No storyboards, SwiftUI, third-party libraries, or network access are needed.

## Data and decisions

- The supplied JSON actually contains **2,698 departures**, five lines, and 20 stations. It is shipped unchanged in `Resources/departures.json`; the built bundle was compared with the original file.
- With no service date in the data, the timetable repeats daily in the device's local time. Times use a consistent 24-hour format. Delay and cancellation messages describe the supplied scheduled departure; delays do not silently change its position in the schedule.
- “Now” means the first scheduled departure strictly after the current local time. If none remains, it shows the end-of-day message. The entire day remains available by scrolling, and past departures are dimmed. The board refreshes time-related styling while open and when returning to the foreground.
- Saved departures span all stations and repeat with the daily timetable. A composite of station, line, destination, minute, and platform supplies a stable identity because the data has no departure IDs. Tests verify that these identities are unique in the supplied file. The station and saved identities persist in UserDefaults.
- Accessibility text sizes use vertically arranged rows and a compact station/filter header. Decorative date/count text is hidden at those sizes to preserve space for departures. Destinations, platform labels, disruption messages, and station names wrap rather than truncate.

## What was hard

The most significant issue was scrolling a self-sizing table across many hours at the largest text size. Estimated heights could put Now at the wrong hour. Row heights are now measured and cached for the current width and text size, and hour-header heights are known precisely. The resulting hour headers remain pinned while their departures scroll.

The starter has no Xcode project, so a small standalone UI-testing project was added under `Tests/`. Native search's content-unavailable view was visually correct but did not expose its message in the simulator's accessibility snapshot; the empty station table now also announces the search-result message through its accessibility label.

## Lookups and verification

I read the starter README and inspected the JSON schema, counts, statuses, time range, and identity uniqueness. Local `simctl` and `xcodebuild` help supplied the appearance, content-size, screenshot, and test-diagnostic options. No web research was needed. Xcode's failure-diagnostic collection lingered after early test failures, so subsequent runs used `-collect-test-diagnostics never`.

- Built, installed, and launched with `SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run`.
- Final normal-size suite: **four tests, zero failures**. It covers the entire dataset, every station/line combination, chronological grouping, strict Now boundaries, midnight, station search and empty results, filtering, saving/removing, cross-station Saved details, relaunch persistence, keyboard use, and long station names. Results: `build/FinalChecks.xcresult` and `build/final-checks.log`.
- Dark mode at `accessibility-extra-extra-extra-large`: the UI journey passed, including visible next-departure positioning, station search with the keyboard, a long station title, filters, Saved, and scrollable empty states. Results and screenshots: `build/LargeDarkFinal.xcresult` and `build/large-dark-attachments/`.
- Simulator logs were checked for unsatisfiable Auto Layout constraints. No such warnings were found during the accessibility checks.
- Only the assigned simulator was used. Its appearance and text size were returned to light/large. The finished app is left installed, with Harbor selected and test saves removed. Final screenshot: `build/finished.png`.

## Remaining work and what would help

No known unfinished specification items. A UIKit starter with a ready-to-run XCTest target, plus an explicit departure ID and service-day/time-zone definition in the dataset, would have made this faster.
