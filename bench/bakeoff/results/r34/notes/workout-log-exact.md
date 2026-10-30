# Workout Log: notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` tests). It is installed on
simulator 498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.workoutlog.b892508`, named "Workout Log".
`bun exact.mjs verify ios` is clean: 5/5 tests pass, no findings in light or dark, nothing spills at the largest text
size, and the scrolling check finds nothing to fix.

## What's in it
- **Three tabs, each with its own stack:** History, Workout and Exercises. Past workouts and exercises are
  routes, so any tab can push them.
- **History:** grouped by month, newest first (by date, then the later-logged on the same day). Each row shows
  the name, date, number of exercises and total volume. A workout's screen shows the date, exercise, set and
  volume totals, then each exercise with its sets. You can delete a workout from there; it asks first.
- **Workout:**
  - Start a workout from an empty state or History's + button. A sheet asks for the name and offers suggestions.
  - Add exercises from a multi-select sheet with search, a muscle menu, and sections by muscle. Exercises
    already in the workout show as added and can't be picked again.
  - Add a set in a sheet with reps on a numeric pad and weight in lb on a decimal pad. Bad input is explained
    and Save stays disabled until it's fixed. A new set starts with the previous set's values.
  - Tap a set to edit or delete it, or swipe to delete. Swipe an exercise to remove it.
  - Rename the workout inline. Finish or discard (discard asks first).
  - Finish drops exercises that have no sets. A workout with no sets at all is refused with an alert.
    Finishing switches to History and opens the new workout.
- **Exercises:** the library of 120, grouped by muscle and searchable. An exercise's screen shows the best set
  (heaviest weight, ties go to more reps) and every workout it appears in, each linking to that workout.
  Exercises never logged say so.
- **Persistence:** both data files ship with the app (bundled by importing them in `app.ts`). History is
  copied from the shipped file on the first write and kept in `app:/data/history.json`. The workout in
  progress is in `app:/data/current.json`, and its name is in a `persist` state. Both survive a relaunch, and
  a test covers that.

## What was hard / what I had to look up
- **`params(nav, …)` takes a parameter name, not a route name.** `params(nav, "exercise")` returned `[]`
  while the route's param was `:id`, so the exercise stats never loaded. I renamed the param to `:exercise`.
- **The data module has a 100 ms CPU budget per turn on iOS.** `Intl.DateTimeFormat` and `structuredClone`
  over 60 workouts went over it (about 150 ms) on the phone only; the web was fine. The resource then failed
  silently into its old value. I now format dates by hand and use plain string compares instead of
  `localeCompare`. I only found this in the drive's `logs`.
- **Things the compiler refused:**
  - A `dialog` inside a grouped `list`.
  - A `footer` under an `if`.
  - `xs[i]` (you write `xs.at(i)` and a `match`).
  - `font-weight="bold"` (it needs `700`).
- **A sheet at `navigationDetent="medium"` with an autofocused field broke an iOS test.** The keyboard grew
  the sheet and left a dimming view over its Save button, so I made it a full sheet.
- **verify's relaunch check reports "lost" when a test ends on a pushed screen.** Navigation isn't restored
  on relaunch, so the check flagged it even though no data was lost. Ending those tests at a tab root cleared it.

## Guesses
- Weight 0 means bodyweight and is shown as "10 reps". Reps are whole numbers from 1 to 999; weight is 0 to
  2,000 lb.
- A finished workout is dated the day you finish it.
- The best set is the heaviest weight; a tie goes to more reps.

## Not done
- No editing of a workout after it's in History; you can only delete it.
- No rest timer and no unit switch (lb only, as the spec says).
- On the web only, a few rows clip at the largest text size. iOS, the target, has no such findings.

## What would have made it faster
- A warning in the compiler or the docs that `params()` names a route parameter, or a type error when the
  name matches no parameter.
- A warning on the web build when a data source is near the iOS CPU budget, or a note that `Intl` is costly
  there.
