# Workout Log: notes

Built with Exact from the `tabs` pattern. It is installed on simulator 498D6D1F-C21C-4B31-90F4-5D857216E256
as `dev.bakeoff.workoutlog.b3df49f`. The 5 tests in `app.test.contract` pass on web and iOS.
`verify ios` is clean (light, dark, the largest text size, scrolling "like UIKit") apart from two
compiler warnings, covered below.

## What's there
- **Three tabs: History, Workout, Library.** Each tab keeps its own navigation stack.
- **History:** lists workouts newest first. Each row shows the name, date, exercise count and
  total volume. A workout in progress appears as its own section at the top, and + starts a new one.
- **Workout detail:** shows a summary (date, exercises, sets, volume), then one section per
  exercise listing its sets, with that exercise's volume in the footer. Tapping an exercise opens
  its screen.
- **Workout tab:**
  - With no workout in progress it shows an empty state with Start Workout. That opens a sheet
    where you name the workout, with suggestions.
  - In a workout you can rename it in place. Add Exercises opens a multi-select sheet: it is
    searchable, has a muscle filter menu in the bar, and is grouped by muscle. Exercises already
    in the workout are marked and can't be added again.
  - Add Set opens a sheet pre-filled with the exercise's last set. Tapping a set edits it in the
    same sheet, which also has a Delete button. Sets swipe to delete, and so do exercises.
  - Finish is disabled until there is at least one set. Exercises with no sets are dropped when
    you finish. The workout then appears at the top of History and the app switches to that tab.
  - Discard (in the ⋯ menu) asks before discarding.
- **Library:** grouped by muscle, with search. Rows show the equipment, which tells apart the
  three exercise names that appear twice in the data.
- **Exercise screen:** shows the muscle and equipment, the best set and every workout the
  exercise appears in (sets and top weight). The best set is the heaviest weight; on a tie, the
  most reps, and the newest of those. Exercises never logged say so; 8 of the 120 are like that.
- **Persistence:** the shipped `data/*.json` files are bundled into the app through `app.ts`
  imports. Finished workouts are saved to `app:/data/workouts.json` and the workout in progress
  to `app:/data/current.json`. Every edit is a queued mutation that reads, changes and writes the
  file, so both survive a relaunch. A test and `verify`'s relaunch confirm it.

## What was hard / what I guessed
- `Entry` is a name reserved by `routes`, so my shape became `LogEntry`. The compiler said so
  clearly.
- My first set sheet put its header inside a child component, following the shape of the
  `edit-sheet` recipe. On iOS that header didn't become a native navigation bar: the X was plain
  and the title the wrong size. I moved the form state to the root, with an `openSet` action that
  fills it and pushes the sheet, and then the bar was native. I couldn't tell from the docs
  whether that is expected.
- Footers can't sit under an `if`. The compiler told me to use one footer with conditional text
  instead.
- At the largest text size the history rows didn't wrap. The fix that worked was
  `min-width="50%"` on the title column inside a `flex-wrap` row; `10rem` and `0` both clipped.
- An exercise route declared only under the History tab reloads into the History tab when you
  reach it from Library, which web `verify` flagged. I added Library copies of the exercise and
  workout routes and pick between them by tab.
- Guesses:
  - Weight 0 is allowed (bodyweight) and reps must be a whole number of 1 or more.
  - A workout finished today is dated today, in local time.
  - Workouts on the same day are ordered with the most recently finished first.

## Not finished / known
- Two `lower-key-position` warnings remain. Set rows are keyed by position because sets have no
  id in the data. The rows hold no state, so this is harmless.
- There is no way to edit or delete past workouts; the spec doesn't ask for it.

## What would have made it faster
- A pattern containing an editable nested record: a parent with child rows edited through a
  sheet.
- A note in `edit-sheet` saying whether a header inside a child component becomes the native bar.
