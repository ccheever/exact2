# Workout Log: build notes

## What was built
Three tabs, each with its own navigation stack:
- **History**: past workouts, newest first. Each row shows the name, date, exercise count and total volume (reps × weight). Workouts on the same day show the one finished last first. A row opens the workout: its totals, then a section per exercise with its sets. An exercise row opens the exercise's screen. A "+" button starts a workout. While one is in progress, a "Resume" row sits at the top.
- **Workout**: the workout in progress. When none is in progress, an empty state with a Start button. Otherwise it shows:
  - a name field
  - a section per exercise: swipe or long-press to remove it; tap it to see its best set and history
  - set rows: tap one for the edit sheet, swipe to delete
  - an "Add Set" button. A new set starts from that exercise's last set.
  - "Add Exercise": a sheet with search and a muscle filter, a pull-down menu in its bar
  - a running summary in the footer
  - Finish, disabled until the workout has a name and at least one set, with the reason shown in the footer. Exercises with no sets are dropped when you finish. Finishing switches to History.
  - a "…" menu with Discard, which asks first
- **Exercises**: the library grouped by muscle, with search. Each exercise's screen shows its muscle and equipment, and its best set: heaviest weight, then most reps, then most recent. Tapping the best set opens its workout. The screen also lists every workout it appears in, with set count and top weight.
- **Persistence**: `data/exercises.json` and `data/history.json` are imported by `app.ts`, so both ship in the bundle. Finished workouts and the workout in progress go into `app:/data/log.json`, rewritten on every edit, so both survive a relaunch.
- **Tests** (`app.test.contract`, 4 tests): history order and totals; best set; the full flow (name, filter, search, add, edit, delete, relaunch, finish, relaunch); discard.
- **Checks**: `bun exact.mjs verify ios` is clean: light, dark, the largest text size and a relaunch.

## What was hard or guessed
- **Bundle id**: Exact's app.json schema refuses `dev.bakeoff.workoutlog.5232df`, because each segment must start with a letter (Apple allows a digit). I built with `dev.bakeoff.workoutlog.w5232df`. For the final install I copied the built `.app` to `.exact/final/`, set `CFBundleIdentifier` (and the id in `receipt.json`) to `dev.bakeoff.workoutlog.5232df`, re-signed it ad hoc and installed it. I then uninstalled the placeholder-id copy.
  - The re-stamped app launches and shows History. The full test suite ran on the placeholder-id build, not this copy.
  - The simulator entitlements linked into the binary still name the `w5232df` application id. That only matters for the keychain, which this app doesn't use.
  - Running `bun exact.mjs ios` again reinstalls the placeholder id. Re-run the re-stamp steps afterwards.
- **Discard alert**: an alert opened straight from a pull-down menu item (`command="show-modal"`) worked on the web but failed on iOS, because the menu is gone before the alert presents. Opening it from an action with `showModal` works on both.
- **Guesses**:
  - Bundled history is read-only (no deleting past workouts).
  - A set needs at least 1 rep (a whole number) and a weight of 0 or more. A blank weight counts as 0, for bodyweight sets.
  - A finished workout's date is the local day it was finished.
- I wrote the date labels by hand rather than with `Intl`, so every JS engine formats them the same.
- Chevrons on push rows are drawn explicitly (the pattern does the same); the host doesn't add disclosure indicators.

## Not finished / known gaps
- The web build still reports clipping at the largest text size in its own tab bar and sheet header. iOS reports nothing there, and iOS is the target.
- There's no reordering of exercises or sets, and no unit switch (pounds only, as the spec says).

## What would have made this faster
- Letting app.json accept any valid Apple bundle id, or a per-platform bundle-id override.
- A note in the menu recipe that an alert from a menu item must be opened from an action on iOS.
