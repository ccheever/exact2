# Workout Log — notes

## What's here
- `Sources/Models.swift`: Codable models. They decode the bundled JSON (integer ids, `"yyyy-MM-dd"` dates, `exercise` key) and the app's own saved format.
- `Sources/Store.swift`: an `@Observable` store. It holds the library, the history (newest first), the workout in progress, best set and appearances, and persistence.
- `Sources/HistoryViews.swift`: the History list (grouped by month, swipe to delete with confirmation) and the workout detail screen.
- `Sources/LibraryViews.swift`: the Library (grouped by muscle, searchable) and the exercise detail screen (best set plus the workouts it appears in).
- `Sources/WorkoutViews.swift`: the Workout tab. It covers starting a workout, the workout in progress, the set editor sheet (add, edit, delete), and the exercise picker (multi-select, search, muscle filter chips).
- `Resources/` holds copies of `data/exercises.json` and `data/history.json`, which ship in the bundle.

## Decisions and guesses
- **Tabs:** History, Workout, Library. If a workout is in progress when the app launches, it opens on the Workout tab, and History shows a "Workout in Progress" row that resumes it.
- **Persistence:** JSON files in Application Support. History is copied from the bundle on first launch and then saved after every change. The workout in progress is saved after every edit, so nothing is lost if the app is killed.
- **Dates:** bundled dates have no time. I parse them as local noon so the calendar day stays the same when the time zone changes. A new workout's date is the time it was started.
- **Best set:** the heaviest weight. Ties go to more reps, then to the most recent workout.
- **Finishing:**
  - Finish is disabled until at least one set is logged.
  - Exercises with no sets are dropped, and the confirmation dialog says so.
  - An empty name becomes "Morning/Afternoon/Evening Workout".
- **Set entry:**
  - A new set is prefilled from the previous set in this workout, or else from the last time that exercise was logged.
  - Reps must be 1–1000. Weight can be empty or 0 for bodyweight, or a decimal such as 17.5.
  - Steppers change reps by 1 and weight by 2.5 lb.
- **Picker:** exercises already in the workout are shown as "Added" and can't be added a second time.
- **Units:** pounds only, as in the spec.

## Testing
I had no UI automation, so I added launch-argument hooks that open a given screen or state for `simctl` screenshots:
- `-openTab`, `-openWorkout`, `-openExercise`
- `-demoActive`, `-showPicker`, `-editSet`, `-finishActive`

They do nothing unless those arguments are passed.

I checked these on the simulator:
- the History totals and the best set, against the JSON (worked out by hand and with a script)
- that a workout in progress survives a relaunch
- that finishing moves the workout to the top of History, survives a relaunch, and drops exercises with no sets
- dark mode and the largest accessibility text size, which led to one layout fix

I couldn't tap through the app, so these were never exercised by real taps: swipe actions, typing in the set editor, the chip filter selection, and the confirmation dialogs. They are standard SwiftUI controls and I checked them by reading the code. After testing I cleared the app data, so the installed app starts from the bundled history.

## What was hard / would have helped
- Without a tap or type tool for the simulator, interaction testing came down to screenshots plus launch-argument hooks. A simple `simctl`-level tap command would have made end-to-end testing much faster.
- `ViewThatFits` didn't switch to the vertical layout for text that can wrap. I had to switch on `dynamicTypeSize.isAccessibilitySize` instead.
