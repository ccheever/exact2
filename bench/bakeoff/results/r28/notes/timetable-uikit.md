# Timetable build notes

Finished as a programmatic UIKit iPhone app. Bundle identifier: `dev.bakeoff.timetable.bbf3589`.
The release build is installed on simulator `EEFC0029-1010-45E4-8242-42CF67E16C96`.

## Implemented

- Full-day, time-sorted station departure board with pinned hour headings.
- Searchable station picker containing all twenty stations.
- Initial scroll and Now control targeting the next scheduled departure strictly after the current local time.
- Native line-filter menu, coloured line badges, platforms, delay minutes and cancellation labels.
- Past departures dim automatically; the clock refreshes while running and when returning to the app.
- Star controls and swipe actions, plus a Saved tab that includes each departure's originating station.
- Persistent station selection and saved departures using UserDefaults.
- Light/dark appearance, Dynamic Type through the largest accessibility size, VoiceOver labels, haptics and a custom app icon.
- The original JSON is bundled byte-for-byte. It actually contains 2,698 departures, five lines and twenty stations rather than approximately 6,000 departures.

## Verification

Used only the assigned simulator. Automated checks run inside the simulator's UIKit app via `Sources/AppChecks.swift`, compiled only with `BUILD_MODE=DEBUG`.

The final functional run passed 269 assertions covering complete lists for every station, every line filter, sorting, hour grouping, midnight and 23:59 formatting, strict next-departure selection, empty lists, before/after-service scrolling, search matching and empty search results, actual star-button actions, Saved updates, and status presentation. A second process launch passed both persistence assertions and restored the original user state.

Inspected simulator screenshots in light and dark appearances, at standard and maximum text sizes, including the station picker, the Saved list, its empty state, and delayed departures. Fixed oversized badges, duplicate navigation chrome, empty-state overlap at maximum text size, and scroll anchoring during Dynamic Type changes. Final release screenshot: `build/release-largest-final.png`.

Logs: `build/checks-final.log` and `build/persistence-final.log`. No Auto Layout constraint warnings appeared in these runs. Verified the release binary excludes the debug checks and its bundled JSON matches the original. Removed the departure saved only for the visual preview. Restored light appearance and standard text size before the final launch.

To rerun functional checks:

```sh
BUILD_MODE=DEBUG SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh
xcrun simctl launch --console --terminate-running-process EEFC0029-1010-45E4-8242-42CF67E16C96 dev.bakeoff.timetable.bbf3589 --self-test
xcrun simctl launch --console --terminate-running-process EEFC0029-1010-45E4-8242-42CF67E16C96 dev.bakeoff.timetable.bbf3589 --self-test --verify-persistence
SIM=EEFC0029-1010-45E4-8242-42CF67E16C96 ./build.sh --run
```

## Hard parts, assumptions, and tooling

The hardest parts were preserving scrolling across self-sizing hour sections and accessibility font changes, and keeping large-text empty states within their own scroll area. iOS 26 navigation and tab bars also required care to avoid duplicate titles and unwanted shared backgrounds around custom branding.

Used the skeleton README, compiler feedback and `simctl ui` help; no web lookup or third-party dependency was needed. CoreSimulator required execution outside the filesystem sandbox, which automatic approval allowed.

Assumed the schedule repeats daily and uses the device's local time, since the data contains minutes after midnight but no date or timezone. Times use a transit-style 24-hour clock. Filters affect Now's target. Delayed/cancelled records remain at their supplied scheduled times and remain eligible for saving. After the last listed departure, Now shows the end of the current day's board rather than jumping to tomorrow.

No specified feature remains unfinished. A supplied simulator touch/accessibility automation driver and an existing UIKit app/testing scaffold would have made end-to-end verification faster.
