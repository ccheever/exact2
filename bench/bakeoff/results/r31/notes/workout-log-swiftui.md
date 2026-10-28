# Workout Log — notes

## What's built
- **History** tab: workouts newest first, grouped by month. Each row shows name, date, exercise count and total volume (reps × weight). Swipe to delete, with a confirmation. When a workout is in progress, a "Resume" row appears at the top.
- **Workout detail**: date, exercise and set counts, total volume, then each exercise with its sets and volume. Tap an exercise to open its screen.
- **Workout** tab: name the workout (recent names offered as shortcuts) and start it. Then:
  - Add exercises from a picker: search across name, muscle and equipment, filter with muscle chips, select several at once. Exercises already in the workout are greyed out.
  - Add sets in a sheet with number pads and steppers. New sets are prefilled from the previous set, or from the last time you did that exercise. The keyboard bar has Next and Add buttons.
  - Edit a set by tapping it. Delete it by swiping or from the editor. Remove an exercise from its ⋯ menu (asks first if it has sets).
  - Finish asks for confirmation, leaves out exercises with no sets, adds the workout to History and opens it there. You can also discard the workout.
- **Library** tab: all 120 exercises grouped by muscle, searchable.
- **Exercise screen**: muscle and equipment, best set, and every workout it appears in, with that workout's sets. Both link to the workout.
- **Persistence**: history and the in-progress workout are saved as JSON in Application Support after every change. History is seeded from the bundled `history.json` on first launch. The selected tab is saved as well.

## Edge cases handled
- The library has duplicate names (e.g. two "Machine Standing Curl" with different muscles). Everything is keyed by id, and rows always show muscle and equipment so you can tell them apart.
- 8 exercises never appear in History. Their screens say so instead of showing a best set.
- Best set is the heaviest weight. Ties go to more reps, then the more recent workout.
- Weights like 17.5 keep their decimals, and a comma decimal separator is accepted. Weight 0 or blank counts as a bodyweight set. Reps must be 1–1000 and weight 0–2000 lb, with inline errors.
- Dates are stored as calendar days (`yyyy-MM-dd`) and parsed in the local time zone, so they don't shift a day. Several workouts on the same day are ordered by finish time.
- Finish is disabled until at least one set exists. A blank name becomes "Workout".

## Tested on the simulator
I checked screenshots of every screen in light mode, dark mode and the largest accessibility text size. I also tested finishing a workout and then relaunching: the new workout was first in History, saved to disk, the empty exercise was dropped, and the exercise's best set updated. The app is installed fresh, with only the seed data.

## What was hard / guessed
- `simctl` can't tap, so I couldn't drive the UI directly. To reach screens for screenshots I added launch-argument hooks (`-openTab`, `-openWorkout N`, `-openExercise N`, `-openPicker 1`, `-openSetEditor 1`; see `shot.sh`), and I wrote an `active-workout.json` into the app container to set up an in-progress workout. Normal launches ignore the hooks.
- In a sheet, the keyboard toolbar didn't appear when the field was focused in `onAppear`. Focusing after a short delay fixed it.
- I had to choose the design myself: three tabs (rather than a modal for the active workout), multi-select in the picker, bodyweight meaning weight = 0, and dropping exercises with no sets when you finish.

## Not done / would be nice
- No reordering of exercises or sets, no rest timer, no editing of workouts after they're finished (only deletion).
- What would have made this faster: a way to send taps to the simulator (or XCUITest) so the add-set and finish flows could be tested end to end, not just by setting up state and taking screenshots.
