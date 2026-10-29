# Workout Log — build notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` tests).
Installed on simulator 498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.workoutlog.b9d2e18`.
`bun exact.mjs verify` passes all 7 tests on the web. `bun exact.mjs verify ios` is **clean**: tests,
light/dark checks, the largest text size, relaunch and real-touch scrolling.

## What the app does
- **Three tabs, each with its own navigation stack.** History, Workout (the one in progress) and Library.
- **History.** Workouts are newest first and grouped by month. Each row shows the name, date,
  number of exercises and total volume. A "Workout in Progress" row sits at the top while a
  workout is running. Tapping a workout pushes its detail: date, exercises, sets and volume, then
  each exercise's sets. Each exercise there links to its own screen.
- **Workout.** Start Workout, then name it. Add Exercise opens a sheet with search (the bar's
  search field) and a Muscle picker; an exercise already in the workout shows a check and is
  disabled. Each exercise is a section:
  - Add Set opens a sheet with Reps (number pad) and Weight (decimal pad). A new set is
    prefilled from the previous set, and the sheet validates as you type: reps must be a whole
    number from 1 to 999, weight 0 to 2,000 lb, and blank weight means bodyweight.
  - Tap a set to edit it or Delete Set; swipe a set to delete it.
  - Swipe or long-press an exercise to remove it, after a confirmation.
  - Finish stays disabled until there is at least one set and asks to confirm. Exercises without
    sets are left out, and a blank name becomes "Workout". Finishing switches to History.
  - Discard asks first.
- **Library.** All exercises grouped by muscle, with search. An exercise's screen shows:
  - its best set: the heaviest weight, with more reps and then the most recent breaking ties;
    it also names the workout and date
  - muscle and equipment
  - every workout it appears in, with that exercise's sets and top weight
- **Persistence.**
  - `data/exercises.json` and `data/history.json` ship inside the app: `app.ts` imports them.
  - Finished workouts are kept in `app:/data/workouts.json`.
  - The workout in progress is kept in `app:/data/current.json`, and its name in a `persist` state.
  - Relaunching with a workout in progress reopens the Workout tab.

## What was hard / guessed
- **Data quirks.** Exercise names don't match their equipment ("Bodyweight Incline Lunge" is
  Machine), and three names are duplicated with different ids. I key everything by id and show
  equipment under each name so the duplicates can be told apart.
- **Ordering.** The spec says newest first. I sort by date, then by finish order within a day, so
  I don't rely on the file's order. A workout finished today goes above the shipped history.
- **Guesses.**
  - The muscle filter is a picker row (`select`), not a segmented control: 8 options don't fit.
  - Tapping an exercise adds it and closes the sheet. There is no multi-select.
  - Weights display as `135 lb × 8`, and a 0 lb set as "8 reps, bodyweight".
- **Exact specifics I learned by trial:**
  - `path()` needs a literal route name, so a conditional picks between two `path()` calls.
  - `font-weight` takes a number.
  - A `footer` can't sit inside an `if`.
  - On iOS a pressable `row` isn't tappable as a button; a `button` row is.
  - After a relaunch, a tab that was never selected has no views, which is why a resume task
    selects the Workout tab.
- **Web-only findings left as they are.**
  - At the web's largest text size, the web-drawn tab bar clips the "Workout" label.
  - The add-exercise sheet's rows are reported clipped mid-animation (the step before
    `clock settle`).
  - iOS draws its own tab bar and reports nothing spilling, so I left both alone.

## Not done
- Finished workouts can't be deleted or edited from History; the spec doesn't ask for it.
- No rest timer and no elapsed-time ticker; the workout shows only its start time.

## What would have made it faster
- A pattern combining tabs with a modal "session" flow (a workout or quiz in progress with
  sheets) would have covered most of this app.
- If the web verify had flagged that `row press=` doesn't work on iOS, I would not have needed an
  iOS round trip to find out.
