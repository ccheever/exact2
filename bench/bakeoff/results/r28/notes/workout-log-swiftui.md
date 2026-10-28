# Workout Log — notes

## What's built
- **History** tab: workouts listed newest first, each showing its name, date, exercise count and total volume (reps × weight summed). Swiping a row deletes it after a confirmation. When a workout is in progress, a "Workout in Progress · Resume" banner sits at the top.
- **Workout detail**: date, exercise/set counts and total volume, then one section per exercise with its sets and the exercise's own volume. Tapping an exercise's name opens the exercise screen.
- **Workout** tab: name the workout, then start it. While it runs you can add exercises from a picker that is searchable, has muscle filter chips and allows picking several at once. Each exercise gets "Add Set", and the new set is prefilled from the previous set or from the last time you did that exercise. Tapping a set edits it (reps and weight fields with steppers, input checks, Delete Set). Swiping deletes a set or removes an exercise. Discard asks for confirmation. Finish shows a summary first. It won't save a workout with no sets. Exercises with no sets are left out, and a blank name becomes "Morning/Afternoon/Evening Workout".
- **Library** tab: exercises grouped by muscle (with counts), searchable by name, muscle or equipment.
- **Exercise screen**: muscle and equipment. The best set is the heaviest weight; ties go to more reps, then to the earliest workout. Below that are the workouts it appears in, newest first, each with its sets. There is a message for exercises that haven't been logged yet.
- **Persistence**: history and the in-progress workout are stored as JSON in Application Support. The in-progress workout is saved on every change. On first launch the bundled `history.json` seeds the history (both data files are copied into `Resources/`).

## What was hard, or guessed
- The library has duplicate exercise names with different ids and muscles (e.g. two "Machine Standing Curl"). So everywhere an exercise appears I show "Muscle · Equipment" under its name, and all lookups use the id.
- Dates in the seed are plain `yyyy-MM-dd`, so I read them as local midnight. New workouts store their full start time, which keeps same-day workouts in the right order.
- Weight 0 is allowed (bodyweight). Reps must be 1–1000. Weights accept a comma or a period as the decimal separator.
- At accessibility text sizes the section headers, the history banner, the set-editor steppers and the start screen all needed their own layouts. They got `ViewThatFits`, a vertical stack, padding-based sizing and a reordered layout. The set sheet opens full height at those sizes.
- A `NavigationLink` inside a section header was tinted like a button, and links inside a `Menu` are unreliable. I made the exercise into a row, and the menu item pushes onto a path binding instead.

## Testing
There is no tap/typing tool for the simulator, so I drove the screens with launch-argument hooks in `Sources/Debug.swift` (`-debugTab`, `-debugPush workout:N|exercise:ID`, `-debugSeedActive YES`, `-debugSheet add|edit|picker`, `-debugFinish YES`) through `shot.sh`. They do nothing unless those arguments are passed.

Checked with screenshots: history totals against the JSON, workout detail, exercise best set (checked against a Python calculation), active workout, set editor, picker, finishing (the workout showed up at the top of History), the in-progress workout surviving relaunch, dark mode, and AX5 text size.

**Not exercised by real taps:** typing into search/text fields, swipe actions, and the confirmation dialogs.

## What would have made this faster
A way to send taps and typing to the simulator (or permission to run XCUITest) would have replaced the debug-hook workaround and let me test text entry and swipes directly.
