# Workout Log — notes

## What's built
- **Tabs:** History, Workout (the one in progress) and Library, each with its own NavigationStack and shared routes (workout ↔ exercise).
- **History:** newest first, grouped by month. Each row shows the name, the weekday and date, the exercise count and the total volume. Swipe a row to delete it (with a confirmation). A "Workout in Progress" row appears at the top while you're logging one.
- **Workout detail:** a summary (date, exercises, sets, volume), then a section per exercise listing its sets. Each exercise links to its own screen.
- **New workout:**
  - Name it, add exercises with a multi-select picker (search plus muscle filter chips; exercises already in the workout are marked).
  - Add a set from a sheet with number fields and ± steppers; it's prefilled from the previous set.
  - Tap a set to edit it. Delete a set by swiping, from its context menu or from the edit sheet.
  - Remove an exercise by swiping; you're asked to confirm if it has sets.
  - Finish is disabled until at least one set is logged. The confirmation summarises the workout and warns that exercises with no sets will be dropped. A blank name is saved as "Workout".
  - After finishing, the app opens the new workout in History. Discard asks for confirmation.
- **Library:** grouped by muscle, with search.
- **Exercise screen:** muscle and equipment, the best set (heaviest weight; ties go to more reps, then the earliest) linked to its workout, and every workout it appears in.
- **Persistence:** history and the active workout are JSON files in Application Support, written on every change. On first launch history is seeded from the bundled `history.json`. A workout in progress reopens on the Workout tab after relaunch.
- Supports dark mode and accessibility text sizes: title/date rows stack, the set-editor steppers move below the field, and the name field wraps.

## Hard / guessed
- I couldn't tap the simulator (no UI automation tool), so I added launch-argument hooks for screenshots (`-uiTab`, `-uiRoute workout:ID|exercise:ID`, `-uiSheet picker|set|finish`). I tested restore and finish by writing the app's JSON files directly into its container and with a temporary auto-finish hook (since removed). Gestures (swipes, typing) were not exercised by hand.
- Seed dates are calendar days, so I anchored them at local noon to stop them shifting a day across time zones.
- Weight 0 is treated as bodyweight. Weights are limited to 0–2000 lb with up to 2 decimals, reps to 1–999. Volume is shown rounded to whole pounds.
- The data has duplicate exercise names, so everything keys on `id`.

## Would have made it faster
- A tap/type driver for the simulator (e.g. idb or XCUITest support in the build script).
