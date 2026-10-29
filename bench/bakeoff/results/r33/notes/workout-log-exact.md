# Workout Log: notes

Built with Exact (started from the `tabs` pattern), installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.workoutlog.b4da869` / "Workout Log".
`bun exact.mjs verify` and `verify ios` are clean: 5 tests pass, light and dark, no clipping
at the largest text size on iOS, and scrolling judged "like UIKit".

## What's there
- **History** tab: workouts newest first, showing name, date, exercise count and total
  volume. Search by name. A "Workout in Progress" row at the top when one is running.
  Tapping a workout pushes its detail (date, counts, volume, then each exercise and its sets).
  Tapping an exercise pushes the exercise screen.
- **Workout** tab: "Start Workout" opens an alert that asks for a name. Then:
  - Add Exercise opens a sheet with search and a Muscle menu filter, grouped by muscle.
  - Add Set opens a sheet with Reps and Weight. It is pre-filled from the last set and Save
    stays off until the values are valid, with the reason shown.
  - Tap a set to edit it (or delete it from that sheet), and swipe a set to delete it.
  - Swipe the exercise's title row to remove that exercise.
  - The ••• menu has Rename and Discard (Discard asks first).
  - Finish stays off until at least one set is logged. Finishing saves the workout to
    History, switches to that tab, opens the new workout and plays a success haptic.
- **Library** tab: all 120 exercises, grouped by muscle into sections and searchable. An
  exercise screen shows its muscle and equipment, then its Best Set: heaviest weight, ties
  broken by more reps, with the workout and date. Below that are the workouts it appears in,
  with that exercise's sets, each linking to the workout. Exercises never logged say
  "No sets logged yet."
- **Persistence**: `data/exercises.json` and `data/history.json` ship bundled; `app.ts`
  imports them. Finished workouts and the workout in progress live in `app:/data/log.json`,
  so both survive a relaunch. The tests cover this with `reload`, and verify relaunches too.

## What was hard / guessed
- Exact's state rules: `persist` holds only primitives, so the workout in progress has to
  live in the data module's file, and every edit is a mutation. Each set sheet is a child
  component whose state starts from its props (the `edit-sheet` recipe), so it opens with
  the tapped set's values.
- A few compile surprises: `Entry` is reserved by `routes`. A block `match` takes no `=>`.
  `font-weight` must be a number.
- A new workout's date comes from `exactTime()` plus `performanceNow()`, since the data
  module may not read the clock. Under `agent ios` drives that clock is a fixed fake
  (it showed Jan 1, 2026), which briefly looked like a bug. The tests pin `epoch`.
- Guessed: "best set" means the heaviest weight, with ties going to more reps.
  Exercises with no sets in a workout are dropped when it is finished.
  A finished workout gets today's local date. On the same day as a shipped workout, the
  person's own workout sorts first.
- On the web, `verify` reports tab labels and the Finish button clipped at the largest
  text size. These are web-only renderings: iOS draws native bars and reports nothing
  spilling. I left them.

## What would have made it faster
- A pattern with a form-in-progress persisted to a file (a draft record edited by many
  mutations), and an example of nested shapes (lists in shapes) in the card.
- A note in the card that the agent's `exactTime()` is a fixed date unless `epoch` is given.
