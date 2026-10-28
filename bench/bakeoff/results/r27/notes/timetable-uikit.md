# Timetable build notes

Finished as a programmatic UIKit iPhone app. Bundle ID: `dev.bakeoff.timetable.bd96e0b`.
Built, installed, and left running on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. No other simulator was used.

## Delivered

- Full-day, chronologically sorted departure board with genuinely pinned hour headings.
- Searchable station picker with all twenty stations, keyboard dismissal, and the selected station persisted.
- Now opens/jumps to the first scheduled departure strictly after the current local minute. After the day's last departure it reveals the end of today's board instead of wrapping.
- All-lines and individual-line filters, colored line badges, destinations, platforms, delayed/cancelled statuses, and dimming of past departures.
- Stars and swipe actions, a Saved tab spanning all stations, and persistent saved departures.
- Native navigation, light/dark colors, VoiceOver descriptions, scalable rows, an accessibility-size compact station header, and a scrollable empty state.
- Custom app icon and the original departure JSON bundled without modification.

## Hard parts

Self-sizing table rows and pinned headings require care when scrolling through estimated row heights. The Now action first realizes the destination row, then accounts for its pinned heading. The end-of-day path realizes the final row before calculating the bottom offset. Minute changes also remeasure rows when the next-departure annotation changes.

An integration check caught a UIKit presentation issue: dismissing a station picker while its search controller is active can dismiss only search. The fix dismisses through the sheet's presenter, and the final keyboard/search/selection/dismissal integration test passes.

Largest accessibility text required a different row arrangement and restrained decorative heading sizes to keep destinations, platforms, and statuses useful. Saved empty-state content scrolls when it grows beyond the available space.

## Lookups and decisions

Read the supplied README and inspected the JSON schema; no web research was needed. Used the installed UIKit SDK/compiler and simctl to validate APIs and behavior.

The specification says about 6,000 departures, but the provided file has exactly **2,698**, with five lines and twenty stations. All 2,698 ship intact. Harbor is the initial station. Times use a consistent 24-hour local clock. The supplied `minute` is the schedule's ordering/jump value; delay text is displayed without inventing a replacement time. Cancelled entries remain visible. Saved identities are generated deterministically from positions in the fixed bundled dataset.

## Verification

`Tests/run-checks.sh` rebuilds and runs the simulator checks on the one assigned device, collects reports, restores the starting saved/station state, and ends with a normal app launch.

- `Tests/self-test.txt`: **256 passing checks**, including every station/line combination, ordering, hour groups, unique save identities, persistence, empty searches, midnight boundaries, and actual table visibility for Now/end-of-day.
- `Tests/ui-test.txt`: actual UIKit control actions for filters, Now, stars, Saved, pinned-header geometry, station sheet, active search/keyboard, selection, and dismissal.
- `Tests/persistence-test.txt`: selected station and saved departures survive real process termination and relaunch.
- Simulator screenshots were visually inspected at standard size, dark mode, and the largest accessibility text size; examples are in `Tests/`.

The checks dispatch UIKit actions directly rather than relying on an external UI-testing framework. Test entry points are available only through explicit launch arguments and are absent from the user interface.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A UIKit starter instead of SwiftUI, an existing simulator UI-test harness, and explicit departure IDs in the JSON would have reduced setup and verification work. Simulator commands also required running outside the filesystem sandbox to reach CoreSimulator services.
