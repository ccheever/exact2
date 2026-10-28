# Workout Log: notes

Built with Exact (`app.contract` view, `app.ts` data module, `app.test.contract` with 8 tests).
`bun exact.mjs verify ios` is clean: tests pass, light and dark, the largest text size, and
scrolling like UIKit. Installed on simulator 498D6D1F-C21C-4B31-90F4-5D857216E256 as
`dev.bakeoff.workoutlog.bc7e695` ("Workout Log").

## What's in it
- Three tabs, each with its own navigation stack: **History**, **Workout** (the one in progress)
  and **Library**.
- History lists workouts newest first, showing name, date, number of exercises and total
  volume. When a workout is in progress, a "Workout in Progress" row sits at the top.
- A workout's screen shows date, exercise and set counts, total volume, and each exercise's
  sets with its own volume. Tapping an exercise opens its screen.
- Exercise screen: the best set (heaviest weight; a tie goes to more reps, then the most
  recent), which opens that workout; muscle and equipment; and every workout the exercise
  appears in, with its set count and top set. An exercise that has never been logged says so.
- Workout: Start opens a sheet where you name the workout, with suggested names. Add Exercise
  opens a sheet that is searchable by name, equipment or muscle, grouped by muscle, and has a
  muscle filter as a pull-down menu in the bar. Exercises already added show a checkmark.
  - Add Set opens a sheet with the number pad. It fills in the exercise's last set and checks
    the input, saying why it's wrong: reps must be a whole number from 1 to 1,000; weight
    0–2,000 lb, and empty means bodyweight.
  - Tap a set to edit it; the edit sheet has Delete Set. You can also swipe a set to delete it.
  - Swipe an exercise, or long-press it, to remove it.
  - The ⋯ menu has Rename (an alert with a text field) and Discard (asks you to confirm first).
  - Finish stays disabled, with the reason shown, until at least one set is logged. Exercises
    with no sets are dropped. After finishing, the app jumps to the new workout in History.
- Persistence: the shipped `data/exercises.json` and `data/history.json` are bundled by
  importing them in `app.ts` and are never changed. Finished workouts go to
  `app:/data/logged.json` and the workout in progress to `app:/data/current.json`, so both
  survive a relaunch. A test covers this with `reload`.

## What was hard
- iOS test timing. Taps right after typing, or on an item in a menu, failed on iOS until I
  added `clock settle` because the keyboard or a transition was still moving. They passed on
  the web.
- A menu item using `commandfor=… command="show-modal"` couldn't open the alert on iOS: the
  alert's source, the menu, had already gone. Calling `showModal` from an action fixed it.
- Contract rules I hit at compile time: no `.some()` (I wrote `map(...).includes`), a block
  `match` uses no `=>`, `Entry` is reserved by `routes`, a section's `header` must come first
  and its `footer` must be last and not inside an `if`, and `font-weight` must be a number.

## Looked up or guessed
- Looked up: the date comes from `exactTime()` plus `performanceNow()`. Tests pin `epoch`
  (without it, a finished workout got a 1970 date and sorted to the bottom).
- Guessed: the date format ("Sun, Oct 25, 2026"), weights shown as "135 lb × 8" (sets with
  weight 0 show as "N reps"), and that new workouts are stamped with today's local date.
- Three library exercise names appear twice with different ids. Everything is keyed by id,
  so they stay separate.

## Not done
- Workouts in History can't be deleted or edited (the spec doesn't ask for it).
- No timer or rest timer.
- The web build's largest-text check flags clipped labels on the web only (tab labels, Finish,
  some picker rows) and an overflow while a sheet is open. iOS's check at the same size finds
  nothing that spills, so I left them.

## What would have made it faster
- A pattern combining tabs with a sheet-based editor of nested records, since that's the
  usual shape for a logging app.
- `verify`/`test web` warning when a step will need `clock settle` on iOS, so it doesn't
  first show up as an iOS-only failure.
