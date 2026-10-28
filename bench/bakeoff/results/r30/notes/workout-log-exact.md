# Workout Log: notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` tests).
It is installed on simulator `498D6D1F-…E256` as `dev.bakeoff.workoutlog.b71f510`, "Workout Log".
`verify ios` is clean: 6 tests pass, no light or dark findings, nothing spills at the largest text
size, and scrolling has nothing to fix. The web tests pass too.

## What's in it
- **Tabs:** History, Workout and Exercises, each with its own navigation stack.
- **History:** newest first, in sections by month. Each row shows the name, date, number of
  exercises and total volume. A "Workout in Progress" row at the top opens the current workout.
  The + button starts a workout, or resumes the one in progress.
- **Workout detail:** a summary (date, exercises, sets, volume), then each exercise with its sets.
  Tapping an exercise opens its screen.
- **Workout tab:**
  - With no workout going it shows an empty state and a Start button. Start opens a name sheet
    with suggestions; Start stays disabled until there is a name.
  - Add Exercise opens a sheet you can search, with a Muscle menu to filter by. An exercise
    already in the workout shows a checkmark and can't be added again.
  - Add Set opens a sheet with weight and reps fields, filled in from the exercise's last set.
    Bad values are explained and keep Save disabled: reps must be a whole number from 1 to 999,
    weight from 0 to 2,000 lb.
  - Tap a set to edit or delete it. Swipe a set to delete it, or swipe an exercise to remove it.
  - Finish stays disabled until at least one set is logged. Exercises with no sets are dropped
    when you finish. Finishing switches to History with the new workout open.
  - Discard asks to confirm first.
- **Exercises tab:** grouped by muscle, with search. An exercise's screen shows its best set
  (the heaviest weight; a tie goes to more reps), its muscle and equipment, and every workout
  it's in, each of which opens.
- **Persistence:** the two shipped JSON files are imported into `app.ts`, so they're bundled.
  Finished workouts go to `app:/data/logged.json` and the workout in progress to
  `app:/data/current.json`, so both survive a relaunch (covered by tests).

## Hard parts, guesses
- **Guesses:**
  - The workout's date is the day it was started.
  - A bodyweight set (0 lb) shows as "12 reps".
  - Volume shows one decimal when it has one (6,767.5 lb).
  - Three exercises share a name, so everything is keyed by id.
- **Navigation:** where to put screens was the main design question. `go()` can jump to another
  tab whose top is the same URL, so rows use `push` from a button instead of a `link`.
- **Sheets:** they had to be components whose root column carries the route's `navigationKey`;
  otherwise their header isn't the bar (the compiler warned).
- **Compiler errors I hit:**
  - A block `match` takes no `=>` on its `case` lines.
  - `font-weight` is a number.
  - A section's `footer` can't sit inside an `if`.
- **Dumbbell icon:** an SF Symbol warns because web and Linux ship too. I added an `app.tamber`
  icon token with an SVG fallback.
- **Web-only findings:** the web `verify` at the largest text size reports clipping in the tab
  bar and the header buttons. On iOS those are UIKit's own bars, and `verify ios` is clean, so I
  left them. I also left two findings in the picker: rows cut off at the list's scroll edge.

## Not done / could be better
- You can't delete or edit a workout once it's in History; the spec doesn't ask for it.
- There's no rest timer and no "previous best" hint in the set sheet.

## What would have made it faster
- A pattern with nested data edited through sheets (a "logbook" pattern) would have been close
  to this app out of the box.
- The `go` vs `push` behaviour across tabs could be written up in the tabs pattern.
