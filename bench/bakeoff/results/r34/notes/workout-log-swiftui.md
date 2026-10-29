# Workout Log — notes

SwiftUI app, iOS 26, no third-party code. Tabs: **History**, **Workout**, **Library**.

## What's in it
- **History**: workouts newest first (by date, then id, so a workout finished today goes above older ones), with name, date, exercise count and total volume (Σ reps × weight). Swipe to delete, with a confirmation. A banner links to the workout in progress.
- **Workout detail**: summary (date, exercises, sets, volume), then a section per exercise with its sets and volume. Each exercise links to its library page.
- **New workout**: you name it (with suggestions; Start stays disabled until there is a name) → "Add Exercises" sheet (search across name, muscle and equipment; muscle filter chips; pick several at once; ones already in the workout are marked) → Add Set / tap a set to edit / swipe or long-press to delete. The set editor has number fields plus steppers (±1 rep, ±2.5 lb), a keyboard toolbar with Next/Done, validation (reps ≥ 1; weight ≥ 0, so 0 works for bodyweight), and a new set starts from the previous set's values. Finish is disabled until there is at least one set. If some exercises have no sets, it asks first and then leaves them out. Discard asks for confirmation. There is also a running timer and live volume.
- **Library**: 120 exercises in sections by muscle, with counts and search. **Exercise page**: muscle and equipment, best set (heaviest weight; ties go to more reps, then the most recent), and every workout it appears in, newest first, with that exercise's sets. 8 library exercises never appear in the history, so their pages show empty states.
- **Persistence**: on first launch, history is seeded from the bundled `history.json`. After that it is saved to `Documents/history.json`. The workout in progress is written to `Documents/active-workout.json` on every change, so it survives being killed at any point. If one exists at launch, the app opens on the Workout tab.
- Dark mode works. At accessibility text sizes, rows stack vertically, decorative icons are hidden, the set editor opens full height, and the exercise page repeats the full name, which the large title would truncate.

## Hard parts / guesses
- No way to tap in the simulator (only `simctl`). I added `DebugRoute.swift`: `SIMCTL_CHILD_WL_*` launch-environment variables open a given tab, workout, exercise or sheet, seed a workout in progress, or finish it. `shot.sh` relaunches with those and takes a screenshot. Normal launches ignore them. Gestures themselves (swipes, typing, tapping chips) were not exercised; I only checked those paths by reading the code.
- `ViewThatFits` didn't fall back to the stacked layout at the largest text sizes, because text wraps rather than reporting that it doesn't fit. I replaced it with an `AnyLayout` that switches on `dynamicTypeSize.isAccessibilitySize`.
- Inside a `Button`, a hierarchical `.primary` style resolves to the tint colour, which turned row text blue. Using `Color.primary` fixed it.
- Guesses: dates are shown as weekday + date (the data has no times). A finished workout gets the date it was *started*. Duplicate exercise names exist in the data (e.g. two "Cable Bulgarian Deadlift"), so identity is always by id and equipment is shown next to the name. Weights entered with "," as the decimal separator are accepted.

## Not done
- No editing of workouts once they're in History (the spec only asks for set editing during a workout), and no reordering of exercises in a workout.
- No UI tests. Checking was by screenshots only.

## What would have made it faster
- A simulator tap/typing tool (or XCUITest available from the build script) to drive real gestures instead of the debug-route workaround.
