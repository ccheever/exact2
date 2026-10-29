# Workout Log: notes

## What's built
- **History tab:** past workouts, newest first, grouped by month. Each row shows the name, date, number of exercises and total volume (reps × weight). You can swipe to delete a workout (it asks first). While a workout is in progress, a "Resume" banner appears at the top.
- **Workout detail:** a summary (date, exercises, sets, volume), then each exercise with its sets and volume. Tapping an exercise opens its screen.
- **New workout:** a full-screen cover where you:
  - name the workout (leave it blank and it gets a name from the time of day, like "Morning Workout");
  - add exercises from a multi-select picker that you can search and filter by muscle using chips;
  - add sets in a sheet with fields and steppers, prefilled from the previous set or from last time;
  - edit a set by tapping it, or delete it by swiping or with the button in the editor.

  Each exercise shows a "last time" footer. You can remove an exercise, minimize the cover to resume later, discard the workout (it asks first) or finish it.
  - Finishing with no sets shows an alert.
  - Exercises with no sets are removed when you finish, after a confirmation.
- **Exercises tab:** the library grouped by muscle, with search. Each row shows how many workouts used the exercise.
- **Exercise screen:** muscle and equipment, the best set (heaviest weight, ties broken by more reps) with a link to its workout, and every workout the exercise appears in, with its sets. If the exercise has never been done, both sections say so.
- **Persistence:** history and the workout in progress are saved as JSON in Application Support after every change. The first launch seeds history from the bundled `history.json`. If a workout was in progress, the app reopens it on relaunch.
- **Display:** dark mode works. At accessibility text sizes the rows switch from side-by-side to stacked layouts.

## Hard / guessed
- `simctl` can't tap, so I couldn't drive the UI by hand. I added a `-screen <name>` launch argument (workout, exercise, unused, library, active, picker, set) that opens a screen directly, and checked each one by screenshot, in light and dark mode and at the largest text size.
- I checked that finishing a workout saves it with a temporary launch hook: the new workout appeared at the top of History and was still there after a relaunch. I then removed the hook and reinstalled the app for a clean state.
- The dates in the data have no time of day, so I stored them as local noon. New workouts keep their real start time, so one finished today sorts above an older workout from the same day.
- Some exercise names appear more than once in the library, so everything is keyed by exercise id.
- Weights are in pounds with steps of 2.5; 0 is allowed for bodyweight. Reps must be at least 1. Either "." or "," works as the decimal separator.

## Not done
- Exercises within a workout can't be reordered, and finished workouts can't be edited (only deleted). The spec asks for neither.
- I didn't check by hand the gestures that need taps: swipes, keyboard focus moves and the minimize-then-resume banner.

## What would have made this faster
- A way to tap and type on the simulator (or XCUITest from the command line), so the flows could be driven end to end instead of opened through launch arguments.
