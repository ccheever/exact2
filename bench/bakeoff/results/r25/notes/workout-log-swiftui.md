# Workout Log — notes

## What's built
- **History** tab: workouts newest first, grouped by month; each row shows name, date, exercise count and total volume (Σ reps × weight). Swipe to delete. A "Workout in Progress" row appears at the top when one is active.
- **Workout detail**: date, exercise and set counts, total volume, and each exercise's sets with that exercise's volume. Exercise headers link to the exercise screen.
- **Workout** tab: name the workout (or pick a suggested name), then:
  - add exercises from a sheet that lets you pick several at once, with search (by name, equipment or muscle) and a row of muscle filter chips;
  - add sets (reps and weight) in a sheet with number/decimal keypads and steppers. A new set is prefilled from the previous set, or from the last time you did that exercise;
  - edit a set by tapping it; delete it by swiping, from the long-press menu, or from the edit sheet;
  - remove an exercise (asks first if it has sets), finish, or discard (asks first).
  - **Finish** is disabled until there is at least one set. Exercises with no sets are left out, and the confirmation says so. After finishing, the app opens the new workout in History.
- **Library** tab: all 120 exercises grouped by muscle, searchable.
- **Exercise screen**: muscle and equipment, best set (heaviest weight; ties go to more reps, then the earlier workout) with the workout it came from, and every workout it appears in with its sets. Exercises that were never logged get an empty state.
- **Persistence**: history and the workout in progress are saved as JSON in Application Support after every change, so both survive a relaunch. History is seeded from the bundled `history.json` on first launch; `exercises.json` is read from the bundle. The selected tab is remembered too.

## Checked on the simulator
- Light and dark mode, and the largest accessibility text size. Rows switch to a stacked layout at accessibility sizes.
- Volume figures checked against the data by hand.
- Persistence checked by seeding an in-progress workout file, relaunching, and finishing it through a temporary hook, then reading the JSON on disk. The hook is removed. The test data is gone too: the app was uninstalled and reinstalled cleanly.

## Edge cases handled
- Some exercise names repeat with different muscle/equipment (for example "Machine Standing Curl"). Everything is keyed by id, and rows show muscle · equipment so the two can be told apart.
- 8 exercises never appear in history.
- Weight 0 is treated as bodyweight and shown as "bodyweight".
- Input is checked: reps must be 1–1000 and weight 0–2000 lb, and decimals are read in the device's locale.
- Dates in the data are plain days ("yyyy-MM-dd"), read as local midnight so they never shift a day. New workouts store a full timestamp.

## What was hard / guessed
- There is no UI-automation tool, so I couldn't tap through the app. I added launch-argument hooks for screenshots instead: `-selectedTab`, `-debugOpen workout:N|exercise:ID`, `-debugSheet picker|addSet|editSet`. They are harmless and left in.
- Guessed: the "Workout" tab (rather than a modal) as the home for the workout in progress; dropping exercises with no sets on finish; naming an unnamed workout "Workout".
- Not done: reordering exercises within a workout, editing finished workouts, and kg units.

## What would have made this faster
- A simulator tap/typing tool (or an XCTest UI runner in the skeleton) for end-to-end interaction tests.
