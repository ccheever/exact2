# Workout Log — notes

SwiftUI, no third-party code. Three tabs: **History** (grouped by month, newest first; swipe to delete with
confirmation), **Workout** (start, then log the workout in progress), and **Library** (grouped by muscle, searchable).
Exercise and workout detail screens can be opened from every tab.

- `Store` (an `@Observable` class) holds the history and the workout in progress. It saves both to
  `Application Support/workouts.json` on every change. On first launch it seeds the history from the bundled
  `history.json`. `exercises.json` is read from the bundle each launch. Both files are in `Resources/`.
- Workout in progress: you name it (with chips suggesting earlier names), multi-select exercises in a picker
  (search plus muscle filter chips), then add sets in a sheet. That sheet has reps and weight fields with −/+
  steppers and prefills from the previous set, or from the last time you logged that exercise. Tap a set to edit it;
  delete with a swipe, the context menu, or the editor. Each exercise has a menu to remove it or open its details.
  You can also discard the whole workout (with confirmation). **Finish** asks for confirmation, drops exercises
  that have no sets, refuses to save a workout with no sets, then switches to History and opens the saved workout.

## Edge cases handled
- Six library exercises share a name with another one (for example, two are called "Machine Standing Curl").
  Everything is keyed by id, and the rows show muscle and equipment so you can tell them apart.
- 8 exercises appear in no workout, so their screens show "Not logged yet" in place of a best set.
- Best set is the heaviest weight; a tie goes to more reps, then to the earlier workout.
- Weights can be fractional (17.5) and you can type `,` or `.` as the decimal separator. 0 lb displays as
  "bodyweight". Reps must be a whole number from 1 to 1000 and weight from 0 to 5000 lb; anything else shows an
  inline error and disables Save.
- History dates are stored at local noon so a day never shifts across time zones.
- Checked in dark mode and at the largest accessibility text size (AX5). At that size, rows and stat grids stack
  vertically and label icons are hidden.

## What was hard, looked up or guessed
- I couldn't tap in the simulator (only `simctl` is available), so I added launch-argument hooks to test the
  screens: `-uiTab`, `-uiRoute w<n>|e<id>`, `-uiDemo 1` (seeds an in-progress workout), `-uiSheet picker|set`
  and `-uiFinish 1`. `shot.sh` relaunches the app with those arguments and takes a screenshot. The hooks do nothing
  unless you pass the arguments. Swipe actions, context menus and the confirmation dialogs were not exercised by
  tapping. They are standard SwiftUI and the logic behind them was tested through the hooks.
- Guesses: volume uses the weight as given, so bodyweight sets add 0. History is grouped into month sections. New
  workouts are dated by start time. Deleting history workouts isn't in the spec; I added it with a confirmation.
- The large-text layouts needed a few rounds of fixes (the History row metadata and the units in the set editor
  wrapped badly).

## Not done
- No app icon, no reordering of exercises in a workout in progress, and no editing of finished workouts. The spec
  asks for none of these.

## What would have made this faster
- A way to tap and swipe in the simulator (for example, idb or XCUITest support in the skeleton) instead of
  launch-argument hooks.
