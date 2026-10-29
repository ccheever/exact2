# Workout Log: notes

Built with Exact, starting from the `tabs` pattern. The app is installed on simulator
`498D6D1F-C21C-4B31-90F4-5D857216E256` as `dev.bakeoff.workoutlog.bfd8295` ("Workout Log").
`bun exact.mjs verify` passes all 6 tests; `verify ios` is clean (tests, light and dark, relaunch,
largest text size, scrolling).

## What's in it
- **History tab**: the 60 shipped workouts plus the ones you finish, newest first (by date,
  then the later one logged on the same day). Each row shows the name, date, exercise count and
  total volume (reps × weight). Tapping a row shows the date, exercise, set and volume totals,
  then each exercise's sets.
- **Workout tab**: Start Workout asks for a name in an alert. You add exercises from a sheet
  with search and a Muscle filter (exercises already in the workout get a checkmark and are
  disabled). Add Set opens a sheet with reps and weight, filled in from the exercise's last set.
  Tapping a set edits it or deletes it, and swiping deletes it. Swiping an exercise's first row
  removes the exercise. Finish is disabled until a set is logged, and the hint says why. Finishing
  drops exercises that have no sets, adds the workout to History and switches to that tab.
  Discard asks you to confirm first.
- **Library tab**: 120 exercises, grouped by muscle, with search. An exercise's screen shows its
  best set (heaviest weight, more reps breaking a tie), its muscle and equipment, and every
  workout it appears in. Tapping one of those workouts opens it.
- **Persistence**: the workout in progress and the finished workouts are saved to
  `app:/data/log.json` after every change. The shipped JSON files are bundled read-only through
  `app.ts` imports. Tests cover relaunching with a workout half-done and after finishing one.
- Input checks: reps must be a whole number of at least 1, and weight must be 0 or more (0
  means bodyweight and shows as "12 reps"). Save stays disabled until both are valid, with the
  reason shown under the fields. `app.ts` refuses bad values too.

## What was hard or guessed
- **The type name `Entry` is reserved** because `routes` declares it, so I renamed my shape.
  The compiler told me so clearly.
- **An alert's text field with `submit=` doesn't close the alert.** I removed `submit` so
  Return presses the alert's Start button.
- **Sets needed stable ids.** The compiler warned about keying rows by position, so sets in
  the workout in progress have ids. The shipped history's sets don't have any, so their ids come
  from their position, which is fine because history is read-only.
- **`navigationDetent="medium"` broke the set sheet.** With the keyboard up the sheet
  expanded during a tap, and the Add button spilled at the largest text size, so I went back
  to a full-height sheet.
- **A section footer can't sit inside an `if`.** It has to be the section's last line, with
  its text chosen by a condition.
- **Guesses:**
  - A workout's date is the day it was started, and finishing keeps it.
  - Two workouts on the same day show the later-logged one first.
  - The same exercise can't be added to one workout twice.
  - Volume is in lb, with no unit setting.
  - Finished workouts can't be deleted. The spec doesn't ask for it, and the shipped history
    is read-only.
- **The data:** 3 exercise names appear twice with different ids, so rows also show the
  equipment to tell them apart.

## Not finished
- Finished workouts can't be edited or deleted afterwards.
- A workout's exercise rows don't link to the exercise's screen (the Library does that).
- Web-only `verify` findings at the largest text size: the web tab bar's "Workout" label and
  the picker rows are clipped there. iOS has the same layout without those problems (its tab
  bar and cells are UIKit's), and the app ships for iPhone.

## What would have made this faster
- A recipe showing an action closing a `dialog`, or an alert prompt whose field's `submit`
  closes the alert.
- A note in the modal recipe that a `medium` detent on a sheet with a field that autofocuses
  is risky.
- `verify ios` takes about 2–4 minutes on a busy machine. The web loop (`test web`, about
  10 s) caught almost everything first.
