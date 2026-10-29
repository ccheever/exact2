# Timetable build notes

## Finished

Built a programmatic UIKit app named **Timetable**, bundle ID `dev.bakeoff.timetable.b75bdf1`, and left the regular build installed and launched on simulator `498D6D1F-C21C-4B31-90F4-5D857216E256`. No other simulator was booted, erased, or used.

Implemented the complete daily departure board with pinned hour headings, searchable station selection, Now navigation, all five line filters, past-departure dimming, delay/cancellation labels, star buttons and swipe actions, and a Saved tab covering all stations. The chosen station and saved departures persist. The clock refreshes while the app runs and on foregrounding, including day changes. Empty states and the after-service case are handled.

The original `data/departures.json` is copied unchanged into `Resources/` and the installed bundle. The supplied file actually contains **2,698 departures**, twenty stations, and five lines, rather than approximately 6,000 departures.

## Hard parts

- Keeping Now positioning accurate while preserving access to the complete day, especially after the last departure and around exact departure times.
- Keeping automatic clock refreshes and star changes from moving the user's scroll position.
- Supporting the largest Dynamic Type size. An initial double-scaling issue was fixed; departure rows switch to a vertical layout, secondary header text has sensible size limits, and empty-state content scrolls so its action remains reachable.
- Testing without an Xcode project or a tap-automation tool. I added an opt-in XCTest-free simulator test configuration that invokes actual UIKit controls and controller callbacks. Test code is excluded from the final regular build.

## Lookups and decisions

No external web lookup or dependencies were needed. I used the supplied README, actual JSON schema, installed compiler, and `simctl`.

- The undated data is treated as the schedule for each local calendar day.
- Times use a consistent 24-hour clock. Harbor, the first station, is the initial choice.
- The `minute` field controls ordering, hour grouping, Now, and dimming. Delay text is displayed exactly as supplied; it does not rewrite the scheduled time.
- Now selects the first scheduled row strictly later than the current local time. Cancelled rows remain in the schedule. After the final scheduled time, Now goes to the end and displays that there are no more departures today.
- Saved trips retain their originating station and remain available when switching stations.

## Verification

- Regular build compiled, signed, installed, and launched successfully.
- **58 of 58 checks passed at standard text size and at the largest accessibility text size.**
- A separate terminate-and-relaunch check confirmed persisted station and saved departure data.
- Inspected simulator screenshots of the board, dark mode, maximum-size rows, station picker, search with keyboard, Saved departures, empty Saved state, and the end-of-day board.
- Confirmed the bundled data matches the source byte for byte and the installed app has the required name and bundle identifier.

Reports and screenshots are in `build/`; the final regular-build screenshot is `build/final.png`. Test instructions are in `Tests/README.md`.

## Unfinished work

None of the requested features remain unfinished.

## What would have made this faster

A supplied simulator UI automation runner with tap, typing, scrolling, and accessibility inspection would have avoided building the in-app verification harness. An accurate record count in the specification and an explicit policy for interpreting the undated schedule and delayed times would also have removed small ambiguities.
