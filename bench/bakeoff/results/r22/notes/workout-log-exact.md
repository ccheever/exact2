# Workout Log: notes

Built with Exact (`app.contract`, `app.ts`, `app.test.contract`). It is installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.workoutlog.beb49be`, "Workout Log".

## What's there
- The app has three tabs (History, Workout, Exercises), and each tab keeps its own navigation stack.
- **History**: past workouts, newest first, in month sections. Each row shows the name, date,
  number of exercises and total volume (the sum of reps × weight). The + button starts a
  workout, or opens the one already in progress.
- **Workout detail**: a summary (date, exercises, sets, volume), then one section per exercise
  with its sets and volume. Each exercise links to its own screen.
- **Workout in progress**: a sheet names the workout. "Add Exercise" opens a sheet you can
  search; its bar button filters by muscle, and the chosen muscle shows as the sheet's
  subtitle. Exercises already in the workout are shown checked and disabled.
- **Sets**: "Add Set" opens a sheet with Reps and Weight fields, pre-filled from the previous
  set. Tapping a set edits it. A set can be deleted by swiping it or from its sheet.
- **Removing an exercise**: swipe its name row, or long-press it.
- **Finish and Discard**: Finish is disabled until at least one set is logged. Exercises with no
  sets are left out of the saved workout. Finishing switches to History. The ••• menu has
  Rename and Discard; Discard asks for confirmation first.
- **Exercises**: the library grouped by muscle, with search. An exercise's screen shows its
  muscle and equipment, its best set and every workout it appears in. Each of those workouts
  opens.
- **Data**: both data files are bundled by importing them into `app.ts`. Finished workouts and
  the workout in progress are each saved in their own JSON file under `app:/data`, so both
  survive a relaunch. The tests check this with `reload`.

## Decisions and guesses
- **Best set**: the heaviest weight; a tie goes to more reps, then to the more recent workout.
  An exercise that's never been logged says "No sets logged yet" (8 of the 120 never appear).
- **Same-day order**: workouts on the same day are ordered with the newer one first; a new
  workout gets a higher id than any existing one.
- **Weights**: weight 0 is allowed and shown as "Bodyweight". Reps must be a whole number from 1
  to 999 and weight from 0 to 2,000 lb. Half pounds such as 17.5 work.
- **Date**: a finished workout is dated with the device's local day.
- **Duplicate names**: some exercise names appear twice in the library, so equipment is shown on
  every row to tell them apart.

## What was hard / looked up
- **Sheets with their own state**: a sheet that edits needs its own component, and that
  component's root has to be the route itself. I first wrapped it in a route column, which
  the compiler refused (`lower-route-place`) or warned about (`lower-header-nested`).
- **Long-press on a link row**: on iOS it failed. The test tool reported a `UIListContentView`
  covering the row. I moved it to a plain `row`, which also gives the swipe.
- **Alert from a menu item**: opening an alert with `command="show-modal"` from a menu item
  failed on iOS because the menu was already closing. An action calling `showModal` works.
- **Set ids**: sets have no ids in the data, so `app.ts` gives each draft set an id. Without
  them, the swipe-to-delete rows would have been keyed by position.
- **Large text (web only)**: at the largest text size, side-by-side text in a row clipped on
  the web, so history rows are now stacked lines.

## Not finished / known
- **Scrolling**: `verify ios` reports scrolling differences from UIKit: lag, tracking and
  momentum. The tool attributes them to Exact itself, since the app has no scroll handlers and
  the rows are a few texts. Everything else in `verify ios` was clean: 7/7 tests, light and
  dark, and nothing spills at the largest text size.
- **Web at the largest text size**: `verify` on the web still reports the web tab bar's labels
  and the Finish button clipping at that size. The iPhone draws the real UIKit tab bar and
  navigation bar, where this doesn't happen.
- **Muscle filter icon**: it doesn't change when a filter is on. An SF Symbol "filled" variant
  would need an icon token for the Linux and web builds; the sheet's subtitle shows the filter
  instead.

## What would have made it faster
- **Pattern**: a pattern with a sheet that edits nested records (here: workout → exercises →
  sets).
- **Card**: the card saying up front that a `link` row can't take `popovertarget`, and that a
  part such as `EmptyState` takes no `testId`.
