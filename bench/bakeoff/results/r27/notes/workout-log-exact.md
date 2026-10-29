# Workout Log: build notes

Built with Exact. `app.contract` is the view, `app.ts` the data module, and `app.test.contract` has 10 tests.
`bun exact.mjs verify ios` is clean: the tests pass, light and dark have no findings, nothing spills at
the largest text size, and scrolling behaves like UIKit. The app is installed on simulator
`498D6D1F-C21C-4B31-90F4-5D857216E256` as `dev.bakeoff.workoutlog.bca9abc` ("Workout Log").

## What's in it
- **Tabs.** History and Library, each with its own navigation stack.
- **History.** A large title. Workouts run newest first, grouped by month, and each row shows the name, date,
  total volume and number of exercises. A row opens the workout's detail: date, exercise count,
  sets, total volume, then each exercise's sets. An exercise there opens its exercise screen.
- **Start Workout** (the bar's + or the first row) opens the workout in progress as a sheet. In it you:
  - name the workout;
  - pick exercises in an "Add Exercises" sheet that has search, a Muscle menu (All or one muscle),
    groups by muscle and lets you check several at once (exercises already in the workout are
    marked and disabled);
  - add a set in a sheet with Reps (number pad) and Weight in lb (decimal pad). A new set starts
    with the previous set's values. Reps must be a whole number from 1 to 999 and weight from 0 to
    2,000. Save stays disabled until both are valid, and the footer says why;
  - tap a set to edit it or delete it (Delete Set);
  - remove an exercise from its ⋯ menu;
  - Discard (asks first) or Finish. Finish is disabled until the workout has a name and one set,
    and the footer says why. Exercises with no sets are left out of the finished workout.
- **Library.** Exercises are grouped by muscle (A–Z), with search. Each row shows the equipment and
  the exercise's best weight.
- **Exercise screen.** The best set is the heaviest weight, with more reps winning a tie. It links to
  the workout it came from. The screen also shows muscle and equipment, and every workout the
  exercise appears in with that workout's sets and best set.
- **Persistence.** The shipped `data/*.json` files are imported into `app.ts`, so they are bundled.
  Finished workouts go to `app:/data/workouts.json` and the workout in progress to
  `app:/data/session.json`, which is written on every edit. After a relaunch, History shows a
  "Workout in Progress" row that resumes it.

## What was hard
- **Sets added after the list was built couldn't be tapped on iOS (the biggest time sink).** These
  were rows with swipe-to-delete (`delete=`), and they could only be reached after a set was added.
  `delete=` makes a row a custom (hosted) cell. In the iOS driver, custom cells inserted after the list
  first renders reported "GroupedCollectionView covers its middle" and could not be tapped. Rows
  present at mount were fine, and so was a plain `button` with two texts, which becomes a native
  value cell. The same failure happened with `button`, with `link` and with `delete=`, and rebuilding
  the section didn't help. A `contextmenu` long-press on a native cell can't be driven either.
  - I never confirmed whether a real finger fails too or only the driver. Either way I dropped swipe
    delete and the long-press menu for sets. You delete a set from its edit sheet, which meets the
    spec ("edit or delete a set").
  - Losing swipe-to-delete is the main compromise compared with a hand-built UIKit app.
- **`verify ios` and the web differ on relaunch.** The web restores the open sheet's URL after a
  reload, but iOS relaunches at the tab root. I wrote the in-progress persistence test against
  History's "resume" row and stored state, so it passes on both.
- **Compile rules I hit:**
  - A section `header` takes exactly one text, so a button can't go in it.
  - A grouped `footer` is one text, so conditions are written as a single ternary.
  - `path()` needs a literal route name.
  - A prop named `filter` shadows `.filter()`.
  - `font-weight` must be a number.
- **A test's finished date.** The finished workout sorted below the seed data until I pinned
  `epoch` in that test.

## What I looked up or guessed
- Looked up: start-here, the card, the `tabs` and `list-detail` patterns, and the recipes for
  `edit-sheet`, `number-field`, `swipe`, `list-rows`, `grouped-form`, `files-and-grants`, `menu`
  and `modal`. I also read Exact's `GroupedListIOS.swift` to understand the hit-test refusal.
- Guessed:
  - The new workout's date is the device's local day.
  - Two workouts on the same day show the later-logged one first.
  - A weight of 0 is shown as "Bodyweight".
  - Three exercises share a name with another exercise, so rows show equipment to tell them apart.
  - Only one workout can be in progress at a time.

## Not finished / known gaps
- No swipe-to-delete on set rows (see above), and past workouts can't be deleted (the spec doesn't
  ask for it).
- The in-progress sheet does not reopen by itself after a relaunch. You resume it from History's
  "Workout in Progress" row.

## What would have made this faster
- A note in the `swipe` / `list-rows` recipes (or a compiler warning) that `delete=` rows become
  custom cells, and that inserted ones may not take taps on iOS. Better still, a fix.
- A way in `agent ios` to tap at a raw point, to tell a driver limitation from a real app bug.
- Clearer documentation of which row shapes UIKit draws natively and which are hosted.
