# Timetable build notes

Finished and installed as **Timetable**, bundle ID `dev.bakeoff.timetable.ba28f73`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. The app is left running with normal text size and light appearance. No other simulator was used.

## Implemented

Programmatic UIKit app and scene delegates; full-day station boards; pinned hourly headers; searchable station selection; Now positioning; single-line/all-lines filtering; dimmed past departures; delayed/cancelled labels; and a Saved tab with persistent departures and station selection. Includes dark mode, VoiceOver descriptions, Dynamic Type layouts, a custom app icon, and the original JSON bundled byte-for-byte.

The provided file contains **2,698 departures**, 20 stations, and five lines, rather than the approximate 6,000 mentioned in the specification. No records were synthesized or dropped.

## What was hard

- Estimated table heights made Now land at the wrong hour with very large text. Cached Auto Layout measurements provide exact row geometry before scrolling.
- An active UIKit search controller has its own presentation. Selecting a station must dismiss the entire station sheet rather than just its search presentation.
- At accessibility sizes, an oversized decorative header crowded out departures. The layout now omits auxiliary header text, uses a native line menu, stacks row details, and keeps full times visible. Empty states can scroll when needed.
- Building UI automation around a swiftc-only starter required a small standalone XCTest project in `Tests/`.

## Lookups and decisions

Read the starter README, build script, JSON structure, and `simctl ui` help. UIKit behavior was verified through builds, UI tests, and screenshots; no web research or external dependencies were needed.

Chose Harbor as the initial station, a 24-hour clock, local device time, and a daily repeating timetable because the data has minutes after midnight but no dates. Now selects the next scheduled minute strictly after the current minute. Status labels preserve the supplied schedule times. Saved services use deterministic identities incorporating all departure fields.

## Verification

- Successful final build, install, and launch using the requested `SIM=… ./build.sh --run` command.
- Final standard-size UI workflow and model tests: **2 tests passed, 0 failures** (`build/UITestFinal`). Covers saving/removing, relaunch persistence, station search including no results, line filtering, Now, midnight, 23:59, empty schedules, and invalid persisted station fallback.
- Largest accessibility text size in dark mode: **1 test passed, 0 failures** (`build/UITestAccessibilityFinal`). Covers correct visible Now target, station picker, native line menu, Saved, and empty Saved.
- Reviewed screenshots in `build/qa-final/`, `build/qa-accessibility-final/`, and `build/dark-final.png`. Final installed screen: `build/final.png`.
- Verified final bundle identity and exact bundled JSON bytes. Final simulator log check found no unsatisfiable Auto Layout warnings.

## Unfinished work and improvements to the starter

No requested features remain unfinished. A supplied UI-test target, stable departure IDs, and explicit choices for default station, clock format, and whether schedules repeat daily would have made this faster.
