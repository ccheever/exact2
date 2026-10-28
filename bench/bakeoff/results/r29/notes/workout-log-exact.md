# Workout Log: build notes

Built with Exact (pin `0b27bed463`), starting from the `tabs` pattern. It is installed on simulator
`498D6D1F-…` as `Workout Log` (`dev.bakeoff.workoutlog.bfaa9bc`). `bun exact.mjs verify ios` is clean:
8/8 tests pass, light and dark have no findings, nothing spills at the largest text size, and the scrolling check found nothing to fix.

## What's in it
- **History tab**: past workouts newest first, each with its name, date, exercise count and total volume.
  An "In Progress" row resumes the current workout. Start Workout is both a list row and the bar's `+`.
- **Workout detail**: the date, exercise and set counts and volume, then each exercise and its sets. An exercise
  row pushes that exercise's screen.
- **New workout**: a sheet to name it (Start stays disabled until it has a name; suggestions are offered). Then
  the in-progress screen: rename the workout, add exercises from a picker sheet (search plus a muscle-filter menu,
  grouped by muscle, already-added exercises checked and disabled), add sets in a sheet (reps and weight, pre-filled
  from the exercise's last set), edit a set by tapping it, delete one by swiping or with Delete Set in its sheet,
  remove an exercise by swiping, and Discard (an alert asks first). Finish stays disabled until the workout has a
  name and at least one set, and the footer says why. Exercises with no sets are dropped when you finish.
- **Library tab**: all 120 exercises, grouped by muscle, with search.
- **Exercise screen**: the best set (heaviest weight; if weights tie, the most reps), with its date, linking to its
  workout. Below it come the muscle and equipment, then every workout the exercise appears in (with its set count
  and top weight).
- **Persistence**: `app.ts` imports both JSON files, so they ship in the bundle. Finished workouts go to
  `app:/data/workouts.json` and the in-progress workout to `app:/data/current.json`, and every edit is written
  straight away.

## What was hard / what I guessed
- The route-entry type is called `Entry`, so my own `shape Entry` was refused. I renamed it to `Lift`.
- Grouped-list footers have to be one `text`, last in the section and outside any `if`. The compiler told me
  exactly how to rewrite them.
- On iOS, a relaunch starts at the root route, while the web keeps the URL. My first relaunch test assumed the
  web's behaviour, so I rewrote it to go through the Resume row, which works on both.
- The background `ios --prebuild` I started before writing the view failed type-checking: `app.ts` referenced
  sources the contract didn't declare yet. The first build inside `verify ios` then rebuilt from scratch (~4 min).
  The lesson: prebuild only after the contract compiles.
- Same-day ordering was a guess. The seed file lists a day's later workout with the lower id, and a new workout
  sorts above older ones on the same date by when it was finished.
- Bodyweight sets use a weight of 0 lb. Reps must be a whole number from 1 to 1000, and weight a number from 0 to 2000 lb.
- Dates show as "Sun, Oct 25, 2026" in the device locale. Weights are always in pounds, as the spec says.

## Not done / could be better
- Past workouts can't be edited or deleted (the spec doesn't ask for it).
- The web-only large-text check flagged four labels clipped on the web (sheet Cancel buttons, long button rows).
  On iOS, the target platform, nothing spills at that size.
- There's no rest timer and no unit switch.

## What would have made this faster
- A pattern that already combines tabs with a modal sheet stacked on a pushed screen. I had to put that together
  from `tabs`, `modal` and `edit-sheet`.
- A warning that a route named `Entry` takes that type name, given before you hit it.
