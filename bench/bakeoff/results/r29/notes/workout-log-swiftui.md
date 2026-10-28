# Workout Log — notes

SwiftUI app with three tabs: **History**, **Workout** (the workout in progress), and **Library**.
Source files: `Models.swift` (types and formatting), `Store.swift` (persistence and logic), and one views file per area.
`data/*.json` is copied into `Resources/` and so ships at the root of the bundle.

## Behaviour and edge cases
- History comes from the bundled `history.json` on first launch. After that it lives in `Application Support/WorkoutLog/history.json`.
- The workout in progress is saved on every change to `active-workout.json`. On relaunch the app opens on the Workout tab.
- Seed dates (`yyyy-MM-dd`) are read as local noon, so they never shift a day across time zones.
- History is sorted newest first. A finished workout is stamped with the time you tap Finish.
- Volume is reps × weight summed. Weights can be fractional (17.5), so up to 2 decimals are shown.
- Three exercise names appear twice in the library (for example "Machine Standing Curl"), each time with a different muscle. Exercises are identified by id, and every list shows a "muscle · equipment" subtitle so the two can be told apart.
- 8 exercises never appear in the history. Their screens say "No sets logged yet" and "Not in any workouts yet".
- The best set is the heaviest weight. Ties go to more reps, then to the earlier workout.
- Finish is disabled until at least one set is logged. Exercises with no sets are left out, and the confirmation dialog says so. An empty name becomes "Workout", and the Start screen picks a time-of-day name.
- An exercise can't be added twice; it shows "Added" in the picker. Removing an exercise that has sets, discarding a workout, and deleting a history workout each ask for confirmation.
- Sets are edited in a sheet with text fields (number pad and decimal pad) plus steppers (±1 rep, ±2.5 lb). Input is checked: reps must be at least 1 and weight at least 0, and both "," and "." work as the decimal separator. A new set is pre-filled from the previous one. Sets are deleted by swiping or from the editor.

## Testing
I can't tap the simulator with `simctl`, so I added launch arguments that put the app in a state and then took screenshots (`shot.sh`):
- `-debugActive 1` seeds a workout in progress.
- `-debugSheet picker|set|edit` opens the exercise picker or the set editor.
- `-debugFinish 1` finishes the workout.
- `-debugTab` and `-debugRoute exercise:40` / `workout:seed-1` open a tab or screen.

With these I checked the History and detail screens, the library, an exercise's screen, the picker, and the set editor. I also checked that finishing adds the workout to History and to the exercise's list of workouts, and that a workout in progress survives a relaunch. Each screen was checked in dark mode and at the largest accessibility text size. I worked out the best set and volume figures separately in Python and they match the app.

The arguments do nothing unless passed. After testing I uninstalled the app to wipe the test data, then reinstalled it clean.

## Hard parts, guesses, gaps
- No tap automation meant the actual gestures were never run: swipe actions, the keyboard toolbar, and the confirmation dialogs are only covered by the code, not by a test.
- In iOS 26 a `.searchable` list inside a TabView hides its search field until you scroll. I set `navigationBarDrawer(displayMode: .always)` so it is always visible.
- Guesses: weight unit is pounds only. Deleting a workout from History is an extra feature. "Workouts it appears in" includes only finished workouts.
- Not done: reordering exercises within a workout (the store supports it but there is no UI), and units other than pounds.

## What would have made this faster
A way to drive the UI from the command line (tap, type, swipe) or XCUITest support in the skeleton.
