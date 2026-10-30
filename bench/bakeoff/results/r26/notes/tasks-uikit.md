# Tasks (UIKit) — notes

## What's built
- UIKit app, set up in code (`AppDelegate` + `SceneDelegate`, scene manifest in `app.env`). No storyboards, no SwiftUI.
- `Store` (Sources/Model.swift) loads the bundled `tasks.json` the first time the app runs, then saves everything as JSON
  in Application Support after every change. Each list keeps the manual order of its task ids.
- Main screen: Today and All, then My Lists with open-task counts. Add a list from the toolbar +. Rename or delete
  a list by swiping, from the context menu or from the list's ••• menu. Delete asks first and says how many tasks
  go with the list. Lists can be reordered in Edit mode. Duplicate or blank names are rejected.
- List screen: open tasks, then a "Completed (n)" header you tap to collapse it. The collapsed state is saved per
  list. The checkbox completes or reopens a task, and the row moves with an animation. Leading swipe also
  completes or reopens; trailing swipe deletes. Long-press and drag to reorder (or use "Reorder Tasks" in the
  ••• menu). Drags stay within the open or the completed section, and the other section keeps its order.
- Today (due today or overdue) and All show the list name on each row and allow complete/edit/delete, but not
  reordering, because they have no manual order of their own.
- Editor sheet: title (wraps), notes, due-date switch with an inline calendar, None/Low/High priority, and a list
  picker (which moves the task). Delete is offered when editing. Cancel or swipe-down asks before throwing away
  changes.
- Marks: an overdue date shows in red with an ! icon and "· Overdue". High priority shows a red "!!!", low a gray
  "!". Due today shows in blue.
- Dynamic Type works up to AX5, with rows that stack at accessibility sizes. Dark mode uses system colors. VoiceOver
  reads each row as one element with a custom complete action.

## What was hard / guessed
- I had no way to tap the simulator, so I added launch arguments (`-open <List|today|all>`, `-edit <id>`, `-new 1`,
  `-scrollEnd 1`) to open screens for screenshots. `-selftest 1` then `-selftest 2` runs the store changes and the
  table's real move-row path, and checks that the data survives a relaunch. All checks passed (log saved in
  `shots/selftest.log`). Because of this, the long-press drag gesture, swipe gestures and the checkbox tap were
  never actually performed by hand. Their code paths go through the same data-source and store methods that the
  self-test does exercise.
- Guesses: "Today" counts open tasks due today or earlier. Lists start in alphabetical order. A new task goes after
  the last open task. Completed tasks are expanded by default. "Today" is the device's current day, recomputed when
  the app returns to the foreground or the day changes.
- Using a diffable data source with reordering needs a subclass that overrides `moveRowAt` and re-applies the
  snapshot itself.

## What would have made it faster
- A way to tap and drag on the simulator (e.g. idb or XCUITest), so gestures could be tested for real.
