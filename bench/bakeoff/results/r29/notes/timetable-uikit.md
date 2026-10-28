# Build notes

Completed Timetable as a programmatic UIKit app. The final production build is installed and launched on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`, with bundle ID `dev.bakeoff.timetable.b8a381d`. No other simulator was booted, erased, or used.

## What shipped

- The complete provided JSON, copied byte-for-byte into the bundle. It actually contains 2,698 departures, twenty stations, and five lines, rather than approximately 6,000 departures.
- The whole station day, sorted by scheduled minute, with pinned hour headings, route badges, destinations, platforms, delayed/cancelled labels, and dimmed past departures.
- Searchable station selection, individual-line/all-line filters, initial positioning at the next departure, and a Now button.
- Star controls, swipe save/unsave, a Saved tab spanning stations, and a departure detail sheet. Station and saved departures persist across process relaunches.
- Adaptive light/dark colors, Dynamic Type through the largest accessibility size, VoiceOver descriptions and activation, and 44-point save targets. Large-text rows rearrange vertically and empty/detail content can scroll.
- A custom transit/clock app icon.

## What was hard

The source order wraps back to midnight after late service, so preserving JSON order would have placed the early departures incorrectly. Sorting happens before hourly grouping. Now navigation also needs defined behavior before first service and after final service; after the last departure it shows the end of today’s board and an explanatory footer.

At the largest text setting, a fixed time column and decorative header would consume too much space. Rows switch to a vertical layout, supporting header text is hidden, and the Saved empty state scrolls. I checked these layouts in simulator screenshots, including long station names.

The sandbox initially prevented CoreSimulator service access. Build/test commands needed the authorized simulator-access escalation. The skeleton also needed an explicit Swift module name for the UIKit scene class and an asset-catalog compilation step for the app icon.

## Lookups and assumptions

I used the local README, build script, data inspection, compiler diagnostics, and `simctl` help for launch logging, appearance, and content-size commands. No web research or third-party dependencies were needed.

I chose Harbor as the initial station, a consistent 24-hour clock, and the device’s local calendar/time zone. The supplied schedule repeats each day. Delays remain attached to scheduled times; cancelled entries remain in the board and in its time-based Now navigation. Saved IDs refer to record positions in this fixed bundled file.

## Validation and remaining work

All requested features are complete. Model checks covered every record, all stations and lines, sorting/grouping, 28,840 time-boundary cases, invalid preferences, and persistence behavior. Simulator integration checks exercised real UIKit control actions, station search/no results, saves, filtering, both persistence values across separate app launches, delayed/cancelled display, dark mode, the largest standard text size, and the largest accessibility text size. Layout logs contained no conflicting-constraint messages. Screenshots and check reports are in `Tests/artifacts`.

The integration harness is in-process UIKit testing plus visual screenshots, rather than an external XCTest touch suite; it is excluded from the installed production build. Test preferences were cleaned to Harbor with no saved departures. A final production bundle check confirmed the app identity, scene manifest, icon, and unchanged bundled data.

A ready-made simulator touch/accessibility test driver and a starter with UIKit lifecycle, scene-manifest, and icon support would have made this faster.
