# Workout Log: build notes

Built with Exact from the `tabs` pattern. The app has three tabs: **History**, **Workout** (the one
in progress) and **Exercises** (the library). It is installed on simulator
`498D6D1F-C21C-4B31-90F4-5D857216E256` as `dev.bakeoff.workoutlog.b5b82f6`. `bun exact.mjs verify ios`
is clean: 5 tests pass, no findings in light or dark mode, nothing spills at the largest text
size, and scrolling has nothing to fix.

## What's where
- `app.contract`: every screen. History is grouped by month, newest first, and each row shows
  the name, date, number of exercises and total volume. Tapping a workout shows its exercises and
  sets, and tapping an exercise there opens the exercise screen. The exercise screen shows the
  best set (the heaviest weight, with ties going to more reps), the muscle and equipment, and the
  workouts it appears in, each linked. The Workout tab has a name field and Start. Once a workout
  is in progress it has Finish, a ••• menu (Add Exercises, and Discard, which asks first), and a
  section for each exercise. In that section you tap a set to edit it, swipe to delete it, or use
  Add Set. You can swipe or long-press an exercise to remove it. Add Exercises opens a sheet with
  search, a muscle filter (a pop-up menu) and multi-select. Add Set and Edit Set open as a sheet
  with numeric and decimal keypads and inline validation. A new set starts with the previous
  set's reps and weight.
- `app.ts`: imports `data/exercises.json` and `data/history.json`, so both ship in the bundle.
  Workouts you finish and the workout in progress are stored in `app:/data/log.json`. Each change
  writes that file straight away, so both survive a relaunch.
- `app.test.contract`: tests for the history order and detail, the whole flow (start, rename,
  filter and search, add sets, edit, delete, finish, relaunch), keeping a workout in progress
  across a relaunch, the library's groups and search, the best set, and discarding.

## Edge cases handled
- Three library exercises share a name, so every row also shows its muscle and equipment.
- Eight exercises never appear in History. Their screen says "No sets logged yet." and "Not in
  any workout yet."
- Finish stays disabled until at least one set is logged. Exercises with no sets are left out
  when you finish, and the footer says so.
- A workout with no name is saved as "Workout". Clearing the name field while a workout is in
  progress puts back the last name.
- A workout finished on the same day as a past one sorts above it.
- A set with weight 0 is a bodyweight set and shows as "10 reps".
- Reps must be a whole number of at least 1, and weight can't be negative. The data module checks
  both again.

## What was hard / what I had to discover
- **A `header` inside a child component is not the navigation bar.** I first put each screen's
  header inside its component (as the `edit-sheet` recipe does). On iOS those headers drew as
  plain content: the title sat under the back button and the sheet's Cancel was a bare link. I
  moved every header into the route's own column. That meant the set form's draft fields moved up
  to root state, filled in by an `openSet` action.
- A route's `header` has to be a direct child of the route column. I found that only by looking
  at screenshots. Neither `contract build` nor `verify` warned about it.
- Opening an alert from a pull-down menu item with `commandfor`/`command="show-modal"` failed on
  iOS ("confirmation is unavailable, transitioning") but passed on the web. A `press` action that
  calls `showModal` works on both.
- Smaller compile errors, each with a clear message: `Entry` is a name `routes` reserves; a state
  initialiser can't read a derive; `path()` needs a literal route name; a `filled` button can't
  be a grouped-list row.
- I guessed at the iOS conventions where the spec says nothing: a tab for the workout in
  progress, sheets for picking exercises and editing sets, months as History's sections, "Top
  N lb" for each workout on the exercise screen, and pounds throughout.

## Not done / limits
- There's no way to delete or edit a finished workout. The spec doesn't ask for it.
- Exercises can't be reordered within a workout.
- The web build's findings at the largest text size (its own tab bar and bar buttons) were not
  all fixed. iOS is the target, and it is clean.

## What would have made this faster
- A warning when a route's `header` isn't the route column's direct child. That one would have
  saved a build-and-screenshot round trip.
- A pattern with a modal form whose fields start from the row that was tapped, with the header
  in the bar. `edit-sheet` puts the header in a child component. In this app that
  layout didn't become the bar on iOS. I didn't test the recipe on its own.
