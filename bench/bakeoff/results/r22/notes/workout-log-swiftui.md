# Workout Log — notes

## What's there
- **History** tab: workouts newest first, grouped by month; each row shows name, date, exercise count and total volume (reps × weight). Swipe to delete (with confirmation). A "Workout in Progress" row appears when one is active.
- **Workout detail**: summary (date, exercises, sets, volume), then each exercise with its sets and per-exercise volume; exercise headers link to the exercise screen.
- **Workout** tab: name a workout (recent names offered), add exercises from a searchable picker with muscle filter chips (multi-select; already-added ones are marked), add sets through a sheet (reps/weight fields plus steppers, prefilled from the previous set or the last session), tap a set to edit or delete it, swipe to delete, remove an exercise, discard the workout, and Finish. Finishing drops exercises with no sets and refuses a workout with no sets at all. Each exercise shows "Last time: …" from history.
- **Library** tab: all 120 exercises grouped by muscle, searchable (name/equipment/muscle).
- **Exercise screen**: muscle, equipment, best set (heaviest weight; ties go to more reps, then the most recent), and every workout it appears in with the sets logged.
- **Persistence**: history plus the in-progress workout are saved as JSON in Application Support after every change. On first launch history is seeded from the bundled `history.json`. If the saved file can't be decoded, it's moved aside rather than overwritten.

## Edge cases handled
- The library has duplicate exercise names, e.g. "Cable Bulgarian Deadlift" (Glutes/Kettlebell and Arms/Cable). Everything uses `id`s, and the picker and workout screens show muscle · equipment to tell them apart.
- 8 exercises appear in no workout. Their screens show "No sets logged yet" and an empty-state message.
- Fractional weights (17.5) are kept as `Double`. Weight input accepts the locale's decimal separator and is checked against 0–2000 lb; reps must be 1–999.
- Seed dates are parsed as local calendar days at noon, so the day shown doesn't shift by time zone.
- At accessibility text sizes, the set rows, the history row metadata and the set-editor fields stack vertically. Dark mode was checked.

## Hard parts / guesses
- No UI automation is available, and `simctl` can't tap. To screenshot the deeper screens I added launch-argument hooks in `DemoHooks` (App.swift) and `-demoSheet` (WorkoutViews.swift), e.g. `-demoExercise 40`, `-demoWorkout 0`, `-demoStart YES`, `-demoSheet edit`, `-demoFinish YES`. They do nothing unless those arguments are passed. `tools/shot.sh` relaunches the app with arguments and takes a screenshot.
- So tap-driven flows (swipe-to-delete, the confirmation dialogs, typing into fields) were checked by reading the code, not by real taps. The store logic behind them was exercised through the hooks: adding sets, finishing, and persistence across relaunch.
- Guesses: a new workout is dated when it's started; "best set" ties are broken by reps; workouts with no sets can't be finished.

## What would have made this faster
- A way to send taps/typing to the simulator (idb or XCUITest support in the skeleton) to drive the real flows end to end.
