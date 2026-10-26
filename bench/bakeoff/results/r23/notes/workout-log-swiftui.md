# Workout Log — build notes

## What's there
- **History** tab: workouts newest first, grouped by month. Each row shows name, date, exercise count and total volume (reps × weight). Swipe to delete, with a confirmation.
- **Workout detail**: a summary (date, exercises, sets, volume) and one section per exercise listing its sets. Tap an exercise header to open that exercise.
- **Workout** tab: name the workout (or tap a suggestion), then start it. Add exercises from a sheet that has search and muscle filter chips and lets you pick several at once. Add a set with a reps/weight editor that has fields, steppers, validation and a pre-fill from the previous set. Tap a set to edit it; swipe, long-press or use the editor to delete it. You can remove an exercise, discard the workout (with a confirmation) or finish it. Finishing leaves out exercises with no sets (and warns first), adds the workout to History and switches to that tab.
- **Library** tab: all 120 exercises grouped by muscle, searchable.
- **Exercise screen**: muscle and equipment, the best set (heaviest weight; ties go to more reps) linked to its workout, and every workout the exercise appears in with its sets.
- **Persistence**: history and the workout in progress are saved as JSON in Application Support on every change. The in-progress workout is reloaded at launch, and the app opens on the Workout tab, which shows a badge. The bundled `history.json` is used only until the first save.

## Edge cases handled
- Duplicate exercise names in the library (e.g. two "Machine Standing Curl"): everything is keyed by id, and the picker shows muscle · equipment so you can tell them apart.
- 8 exercises never appear in history: their screens show an empty state instead of a best set.
- Weights are decimal (17.5): the field accepts both "." and ",", and 0 lb is allowed for bodyweight. Reps must be 1–1000.
- Seed dates are day-only. They are placed at local noon so they never shift to another day.
- Dynamic Type at accessibility sizes: the set editor stacks its fields vertically and opens full height. Long names wrap. Checked in dark mode.

## What was hard or guessed
- There's no UI automation tool, so I couldn't tap through the app. I added launch-argument hooks (`-debugTab`, `-debugPath`, `-debugSheet`, in `DebugLaunch` in App.swift) to open specific screens and sheets for screenshots. I also seeded a draft workout by writing `draft.json` into the app container. To check finish and persistence, I temporarily added a launch flag that finishes the draft, confirmed that `history.json` then held 61 workouts and the exercise with no sets was dropped, and removed the flag.
- I avoided `TextField(value:format:)` because it only commits when focus leaves the field, so pressing Save mid-edit could lose the last keystroke. The fields use text plus my own parsing instead.
- Guesses: the start screen requires a name. Adding the same exercise twice to one workout isn't allowed (it shows as "Added"). Finished workouts are dated when they started.

## Not done / would improve
- The add/edit/finish flows were never tapped through by hand; I checked them by reading the code and through the hooks above.
- No reordering of exercises or sets, and no rest timer.
- A UI driver (tap/type through simctl) would have made testing much faster.
