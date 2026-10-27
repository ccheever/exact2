# Workout Log — notes

SwiftUI, iOS 26, no Xcode project (built with `./build.sh`). Sources:
- `Models.swift`: models, the format of the bundled seed file, and number/date formatting
- `Store.swift`: an `@Observable` store that saves its data
- `HistoryViews.swift`, `LibraryViews.swift`, `ActiveWorkoutViews.swift`, `App.swift`: the tabs, sheets and navigation

Both data files are copied into `Resources/` so they ship in the app bundle.

## How it behaves
- **Tabs:** History, Workout and Library. Each has its own NavigationStack, and screens are pushed with `Route` values, so you can go workout → exercise → workout.
- **History:** newest first, grouped by month. Each row shows the name, date, number of exercises and total volume (reps × weight). Swipe a row to delete it (asks first). When a workout is in progress, a banner at the top links to it.
- **Workout detail:** a summary (date, exercises, sets, volume), then each exercise with its sets and volume. Tapping an exercise opens its screen. The menu offers "Repeat Workout" (starts a new workout with the same exercises) and "Delete".
- **Starting a workout:**
  - A name sheet with suggested names; a blank name becomes "Workout".
  - "Add Exercises" opens a picker: search (name, muscle or equipment), muscle filter chips, and you can pick several at once. Exercises already in the workout show as "Added".
  - "Add Set" opens a set editor pre-filled from the exercise's last set. It has number-pad fields, steppers (±1 rep, ±2.5 lb) and checks the input: reps must be 1 or more, weight 0 or more (0 = bodyweight).
  - Tap a set to edit it. Delete a set by swiping or from the editor.
  - The ⋯ menu on an exercise removes it or opens its history.
  - "Finish" is disabled until the workout has at least one set. It asks for confirmation and leaves out exercises with no sets. "Discard" also asks first.
- **Library:** grouped by muscle, searchable.
- **Exercise screen:**
  - Shows muscle, equipment and how many times it was logged.
  - The best set is the heaviest weight. Ties go to more reps, then to the earliest workout. It links to that workout.
  - Lists every workout the exercise appears in, with its sets, and handles exercises never logged.
  - When a workout is in progress, it has an "Add to <workout>" button.
- **Saving:**
  - History and the workout in progress are written as JSON files in Application Support each time they change.
  - The bundled history is read only until the first change is saved.
  - On relaunch, an unfinished workout reopens on the Workout tab.

## Testing
I couldn't tap or type in the simulator: there's no UI-automation tool, only `simctl`. So I added launch arguments that open a given screen directly: `-debugTab`, `-debugWorkout <index>`, `-debugExercise <id>`, `-debugSheet start|picker|set`. Normal use ignores them. `shot.sh` relaunches the app with arguments and takes a screenshot.

- **Checked with screenshots:** History, workout detail, exercise detail, Library, the in-progress workout, the exercise picker and the set editor. Covered light and dark mode and the largest accessibility text size (AX5).
- **Checked restoring after relaunch:** I wrote an `active-workout.json` into the app's data folder and launched the app; the workout came back.
- **Checked against the data:** the first workout's volume (6,767.5 lb) by hand. I also checked with a script that the data has no broken exercise references, and that 8 exercises are never logged (they show an empty state).
- **Not run through taps:** finishing, editing and deleting were only reviewed in the code.

## Hard parts / guesses
- **Seed dates:** these are plain days. I stored them as noon local time, so time-zone shifts don't move them to another day. New workouts use the time they started.
- **Duplicate names:** two exercise names appear twice in the library, so every row also shows the equipment to tell them apart.
- **Largest text size:** at AX sizes, set rows and the set editor fields switch from side-by-side to stacked (`AnyLayout`). In the Workout tab, a long workout name is cut off with "…" in the one-line name field rather than wrapping.
- **Menu link:** a NavigationLink inside a `Menu` doesn't push a screen, so the exercise menu sets `navigationDestination(item:)` instead.

## Not done
- No units setting (pounds only).
- No way to reorder exercises.
- No rest timer.
- No editing of finished workouts, only deleting them.

## What would have made this faster
A way to tap and type in the simulator from the command line (idb or XCUITest). Without it, every interactive flow needed a launch-argument hook to check by screenshot.
