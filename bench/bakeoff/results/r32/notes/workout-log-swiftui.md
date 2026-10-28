# Workout Log — build notes

SwiftUI, iOS 26, no Xcode project (built with the skeleton's `build.sh`). The bundled `exercises.json` and
`history.json` are in `Resources/`.

## How it's built
- `Store` (`@Observable`) holds the library, the history (sorted newest first) and the workout in progress.
  The history and the workout in progress are saved as JSON in Application Support on every change. The
  history is seeded from the bundled `history.json` on first launch. The app reopens on the Workout tab if a
  workout was in progress.
- Three tabs: **History** (grouped by month, swipe to delete with a confirmation) → workout detail;
  **Workout** (start, name, add exercises, add/edit/delete sets, finish or discard); **Library** (grouped by
  muscle, searchable) → exercise detail (best set, the workouts it appears in). Exercise and workout screens
  link to each other.
- The exercise picker is a multi-select sheet with search and muscle filter chips. Exercises already in the
  workout are shown but disabled, so the same exercise can't be added twice.
- Set editor: reps (whole number ≥ 1) and weight (≥ 0, so 0 works for bodyweight; "." or "," as the decimal
  separator), each with a stepper. The weight stepper moves by 2.5 lb. A new set is pre-filled from the
  previous set of that exercise, or from the last time the exercise was done. The current value is selected
  when a field gets focus, so typing replaces it. A keyboard toolbar shows Next/Add.

## Decisions and guesses
- **Best set** = heaviest weight. Ties go to more reps, then the most recent. It counts finished workouts
  only.
- **Volume** = Σ reps × weight, shown with at most one decimal and grouping separators ("6,767.5 lb"). I
  checked it by hand against the data.
- Seeded workouts only have a date, so they are placed at local noon. New workouts are dated when they start.
- Finishing drops exercises that have no sets, and the confirmation says so. Finishing with no sets at all
  doesn't save; it offers to discard instead. A blank name falls back to "Morning/Afternoon/Evening/Night
  Workout".
- The data has 3 duplicate exercise names, and many names contradict their equipment (e.g. "Dumbbell Wide
  Dip" is Barbell). Everything is keyed by id, and rows show muscle/equipment to tell them apart.

## What was hard / what I couldn't do
- **No way to tap the simulator.** `simctl` can't send touches and no UI tool was installed. To test, I
  added `#if DEBUG` launch-argument hooks (`-debugSheet picker|add|edit|finish|finishNow`, `-debugTab`,
  `-debugExercise`, `-debugOpenFirst`). I also wrote a workout-in-progress file into the app container to
  test relaunch. With those I checked every screen in light/dark mode and at AX-XXXL text, and I checked
  the finish → History write against the saved JSON. The hooks are compiled out of the shipped build
  (`build.sh` doesn't pass `-D DEBUG`). Gestures themselves (swipe-to-delete, the actual tap sequences)
  were never run through a real touch, but they use standard SwiftUI.
- `tools/shot.sh` relaunches the app with arguments and takes a screenshot.

## What would have made it faster
- A tap/type driver for the simulator (idb, or XCUITest without an Xcode project).
