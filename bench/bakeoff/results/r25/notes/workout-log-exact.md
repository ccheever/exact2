# Workout Log: notes

Built with Exact from the `tabs` pattern. Installed on simulator 498D6D1F… as
`dev.bakeoff.workoutlog.b9a634e` ("Workout Log"). `bun exact.mjs verify` (web) passes all 8 tests,
and `verify ios` is clean: tests, light/dark checks, largest text size, scrolling and relaunch.

## What's in it
- **History tab:** workouts newest first (ties on the same day put the most recently logged first),
  each with its name, date, exercise count and volume. A "+" in the bar starts a workout. While one
  is in progress, an "In Progress" row at the top resumes it.
- **Workout detail:** date, exercise count, set count and volume, then one section per exercise
  with its sets. Each exercise row opens that exercise.
- **Workout in progress** (a sheet): a name field, then one section per exercise. Tap a set to edit
  it in a sheet (with Delete Set) or swipe it to delete. "Add Set" opens the set sheet filled in
  with the previous set's values. You can remove an exercise (it asks first if the exercise has
  sets), add exercises, discard the workout (it asks first) and Finish. Finish stays disabled until
  the workout has a name and at least one set, and the footer says why. Exercises with no sets are
  dropped when you finish.
- **Exercise picker** (a sheet): pick several exercises at once (checkmarks, "Add (n)"), with a bar
  search field and a Muscle filter menu.
- **Exercises tab:** the library grouped by muscle, with search. An exercise's screen shows its
  muscle and equipment, its best set (heaviest weight, ties broken by more reps, with the workout
  and date) and every workout it appears in. An exercise that was never logged says so.
- **Persistence:** both data files ship through `app.ts` JSON imports. The workouts you finish go
  to `app:/data/logged.json`, and the workout in progress goes to `app:/data/current.json` after
  every change. If you relaunch with a workout in progress, the app reopens it.

## What was hard, or a guess
- iOS's bar search hides the sheet's navigation bar (Cancel/Add) while the search is active, which is
  UIKit's own behaviour. I found no Exact switch for `hidesNavigationBarDuringPresentation`, so in
  the picker you end the search (✕) and then tap Add. The selection is kept.
- On the web, a reload keeps the current route. On iOS, a relaunch starts at the root. The relaunch
  test passed on one host and failed on the other, so I added a `task … when current.active` that
  reopens the in-progress workout once at launch.
- `verify ios` failed some taps with "native work in flight" (a sheet's detent changing as the
  keyboard came up). I dropped the medium detent on the set sheet and added `clock settle` steps.
- Smaller snags: `Entry` is a name reserved by `routes`; `path()` needs a literal route name, so
  shared detail components choose between two `path()` calls; a state's initializer can't read a
  derive; `font-weight` takes a number. The compiler named each of these with its fix.
- I guessed at things the spec leaves open: weights are shown in pounds as entered (decimals
  allowed, 0 for bodyweight), reps must be whole numbers of 1 or more, and a finished workout is
  dated today in the device's time zone.

## What would have made it faster
- A pattern with a nested data shape kept in a file, plus a sheet-over-sheet flow (a picker over an
  editor). That is most of this app.
- A note in the search docs that the bar search hides a sheet's bar buttons while it is active.
- Web `verify` reported "clipped at the largest text size" for rows that were only scrolled
  outside the viewport mid-test. iOS reported nothing spilling. Telling those two cases apart
  would save some time.
