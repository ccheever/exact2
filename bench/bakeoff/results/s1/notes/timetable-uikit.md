# Timetable build notes

Finished as a programmatic UIKit iPhone app, built, installed, and launched on simulator `CEDFB9C9-FF04-4617-B156-94C56B83CA86` only. Bundle ID: `dev.bakeoff.timetable.b318fa9`.

## What was hard

- Self-sizing departure cells and pinned hour headings needed a second alignment after layout to make Now land accurately, particularly with accessibility text sizes.
- Keeping a useful departure viewport at the largest text size required an adaptive header: the station picker remains at the top, supplementary headings disappear, and departure information reflows vertically. Empty states scroll when their text exceeds the viewport.
- Earlier services need to look dimmed without making destinations, platforms, or disruption messages unreadable. Past services use secondary text and faded route badges while status messages retain clear colors.

## Decisions and things checked

- The supplied JSON contains **2,698 departures**, rather than the specification's approximate 6,000. All five lines, twenty stations, and every supplied departure are bundled unchanged. Both resource copies were compared byte for byte with the source.
- Times use the device's local day and a 24-hour clock. Scheduled minutes determine sorting, hour grouping, dimming, and Now; delay text is shown as supplied. Now chooses the first scheduled minute strictly after the current minute. After the last service, it scrolls to the end-of-day message instead of wrapping to tomorrow.
- Saved includes departures across all stations, ordered by time and grouped by hour, with each departure's originating station shown. Favorites and station selection use UserDefaults and stable departure identities.
- UIKit's native navigation, tabs, search controller, sheet, keyboard handling, accessibility controls, and swipe actions are used. No SwiftUI or storyboard is present. The icon is reproducible with `Scripts/make_icon.py` (Pillow).
- No web research or third-party runtime dependencies were needed. SDK compiler feedback and simulator behavior resolved UIKit API and layout details. Simulator operations needed execution outside the filesystem sandbox to reach CoreSimulator services.

## Validation

- Final build command succeeded: `SIM=CEDFB9C9-FF04-4617-B156-94C56B83CA86 ./build.sh --run`.
- In-app checks: **181/181 passed** at normal text size and **182/182 passed** at maximum accessibility text size, in dark mode. Checks cover bundled data, every station and line filter, time ordering, midnight and end-of-service boundaries, star controls, Saved synchronization, station selection, case-insensitive/trimmed search, empty search results, persistence, and accessibility layout.
- A separate seed / exit / relaunch / verify sequence confirmed the chosen station and three saved departures survived a real process restart. Test preferences were restored afterward.
- Visually inspected light and dark mode, the maximum accessibility text size, the Now target under pinned headers, station search with the keyboard, saved departures with delays/cancellations, and the Saved empty state. Final screenshots and a check summary are in `build/verification/`.
- The generated Info.plist passed `plutil` validation. The final app is installed and open with the original normal text size and light appearance restored.

## Remaining work

No known unfinished specification features. Validation was limited to the designated simulator, as requested; no physical-device or automated VoiceOver gesture testing was performed.

## What would make this faster

A supplied XCTest/UI automation target or simulator tap/scroll driver would make interaction testing faster than a launch-driven in-app check harness. A reference design and explicit rules for delayed-service ordering and end-of-day behavior would remove the few product decisions above.
