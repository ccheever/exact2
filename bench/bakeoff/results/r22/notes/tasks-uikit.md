# Tasks (UIKit) — notes

## What's there
- `Sources/AppDelegate.swift` — UIKit app/scene delegate (scene manifest in `app.env`), plus a small
  launch-argument hook (`-debugScreen`, `-debugTask`, `-debugActions`) I used to reach screens for screenshots.
- `Store.swift` — Codable store in Application Support (`tasks-store.json`), seeded on first launch from the
  bundled `tasks.json` (copied into `Resources/`). Saves after every change. Lists have stable UUIDs, so
  renaming a list doesn't touch its tasks. Due dates stay as `yyyy-MM-dd` strings in the local calendar.
  Comparing those strings directly avoids time-zone bugs.
- `ListsViewController` — Today / All counts, then My Lists with open counts. Add List (alert with a name
  check that rejects empty and duplicate names), rename and delete (swipe, context menu, or edit mode;
  deleting asks first and says how many tasks go with the list), and drag to reorder lists in edit mode.
- `TasksViewController` — diffable inset-grouped table. A list shows open tasks, then a "Completed (n)"
  section with a header button that collapses it; the collapsed state is saved per list. Today shows
  Overdue and Today sections; All groups by list. The checkbox (and a leading swipe) completes or reopens a
  task with an animated move. A trailing swipe deletes. Reorder by long-press drag or Edit + grab handles,
  limited to the open section of a real list. The "…" menu has Show/Hide Completed, Rename List and Delete List.
- `TaskEditorViewController` — a modal form: title, growing notes field, date switch with an inline
  calendar, priority and list pop-up menus, and Delete. Save is disabled while the title is empty. If there
  are unsaved changes, swipe-to-dismiss and Cancel ask before discarding.
- Marks: overdue dates show in red with an exclamation icon and the word "Overdue". High priority shows an
  orange "!!!" (low shows a grey "!"), as Reminders does. VoiceOver labels and custom actions are included.
- Checked in light and dark mode and at the largest accessibility text size (AX5).

## What was hard / guessed
- I had no tap automation for the simulator: `osascript` / System Events hung, probably because it lacks
  accessibility permission. Instead I added debug launch arguments that open screens and run toggle,
  collapse, edit-mode and move actions, then took screenshots. Through these I checked toggling, persistence
  across relaunch, reordering, the collapsible section, edit mode, dark mode and AX5. I could **not**
  physically test: long-press drag, swipe actions, the list name alerts, menus, or typing in the editor.
  That code follows standard UIKit patterns, but no one has run it by hand.
- Combining a diffable data source with drag reordering: the data source subclass handles
  `moveRowAt`/`commit`, and after a move the snapshot is rebuilt without animation.
- Judgment calls: "Today" means open tasks due today or earlier. Its Completed section holds done tasks due
  on or before today, so a task you check off moves there instead of disappearing. Completed starts expanded
  in lists and collapsed in Today/All. Lists are seeded in alphabetical order. Tasks in a list keep the
  file's order until you reorder them.
- At AX sizes the toolbar buttons stop growing at accessibilityMedium, and the checkbox is capped at 36 pt.

## What would have made it faster
- A tap/swipe driver for the simulator (e.g. `simctl io … tap x y`), or permission for XCUITest.
