# Workout Log: notes

Built with Exact. The app has three tabs:

- **History** lists past workouts newest first. Each row shows the name, date, exercise count and total volume.
- **Workout** holds the workout in progress.
- **Library** lists the exercises, grouped by muscle and searchable.

Workout and exercise screens push from both History and Library. Adding an exercise opens a
sheet, and so does adding or editing a set.

All data lives in one JSON file (`app:/data/log.json`). It is seeded from the shipped
`data/history.json` on first launch. Every edit to the workout in progress is written to that
file, so both the history and an unfinished workout survive a relaunch. `verify ios` checked
this.

Status: `bun exact.mjs verify ios` is clean. All 5 tests pass, there are no light or dark
findings, nothing spills at the largest text size, and scrolling is "like UIKit". The app is
installed on simulator 498D6D1F-C21C-4B31-90F4-5D857216E256.

## Decisions I made without asking (the spec is silent)
- **Finishing a workout.** It needs a name and at least one logged set. The reason shows under
  the name field until both are there. Exercises with no sets are left out of the saved
  workout. A finished workout gets today's date and opens in History.
- **Best set.** It is the heaviest weight. A tie goes to more reps, then to the earliest date.
  An exercise that was never logged says "Not Logged Yet".
- **Set validation.** Reps must be a whole number from 1 to 999. Weight can be 0 to 2,000 lb,
  and 0 is allowed for bodyweight. A new set starts with the previous set's reps and weight
  filled in.
- **Deleting and discarding.**
  - Swiping a set deletes it, and so does the Delete Set button in its sheet.
  - Swiping the exercise's first row removes the exercise.
  - Discard Workout asks for confirmation first.
- **Adding exercises.** An exercise cannot be added twice to the same workout. The picker shows
  a checkmark on exercises already added.
- **Same-day workouts.** They are ordered by date, then by creation order (newest first).

## What was hard / what I had to look up
- **Concurrent writes.** Several mutation slots write the same file: renaming on every
  keystroke while a set saves, for example. I added a promise-chain lock in `app.ts` so these
  writes can't lose each other's changes.
- **Workout name field.** The name has to stay editable without the refreshed resource
  overwriting it mid-typing. I solved it with `nameEdit` (an option overlaying `current.name`).
- **Route links in components.** `path()` needs a literal route name, so components take an
  `inLibrary` flag instead of a route name.
- **List rows on iOS.** A `row press=` inside a grouped list is not tappable on iOS, so the
  picker rows went back to being `button`s.
- **Test steps the web accepts but iOS refuses.**
  - Tapping a bar search field works on the web, but on iOS it is a bar item.
  - On iOS, Save has to wait for `clock settle` after typing, because the keyboard is still
    moving.
- **Guessed numbers in my first tests were wrong.** For example, workout 1 has 4 exercises, not
  5. I checked the data with a quick script before writing the final expectations.

## Not finished / known gaps
- **Web `verify` findings.** Web `verify` still lists clipping at the largest text size. It
  comes from the web-drawn tab bar and header buttons, and from picker rows half scrolled out
  of view. On iOS those bars are UIKit's, and `verify ios` reports nothing spills.
- **No extras.** There is no editing of finished workouts, no reordering of exercises, and no
  rest timer. The spec doesn't ask for any of these.

## What would have made this faster
- **Earlier warnings from `contract build`.** It could flag `row press=` in a grouped list
  (it doesn't work on iOS), and test steps that only work on the web.
- **A per-tab route example in the `tabs` pattern.** The pattern could show a detail route
  reachable from two tabs (my `workout` / `libWorkout` pair).
