# Tasks (UIKit) — notes

## What's built
- `Sources/AppDelegate.swift`: UIKit app and scene delegates. The scene manifest is in `EXTRA_PLIST` in `app.env`.
- `Store.swift`: the model and JSON persistence in Application Support (`tasks-store.json`, written atomically on
  every change). On first launch it imports the bundled `Resources/tasks.json`, a copy of `data/tasks.json`.
  The six lists are created in alphabetical order. A task's position in the array is its order within its list.
- `ListsViewController`: Today, All and My Lists, each with its count of open tasks. Lists are added from the
  toolbar. Swipe, the context menu, or tapping in Edit mode renames a list. Swipe, the context menu, or Edit mode
  deletes one, after a confirmation that says how many tasks it holds. Lists can be reordered in Edit mode.
- `TaskListViewController`: open tasks, then a "Completed (n)" section that collapses. The collapsed state is
  remembered per list. Tapping the checkbox completes or reopens a task, and a leading swipe does the same.
  Trailing swipe deletes. Long-press drag reorders open tasks within a list (also possible from "Edit Tasks").
  Today and All show which list each task is in and don't allow reordering, because their order is derived.
- `TaskEditorViewController`: a modal form for adding or editing a task: title, notes, a due date switch with an
  inline calendar, priority, list, and Delete. Save is disabled while the title is blank. Dismissing with unsaved
  changes, by swipe or Cancel, asks before discarding.
- Marks: an overdue task shows a red "Overdue · date" with an icon. High priority shows an orange "!!!" and an
  orange checkbox ring; medium shows "!!" and low has no marker. Completed tasks are never shown as overdue.

## Decisions and guesses
- "Today" means due today or earlier, open or done. Done ones go in its Completed section, and its count counts
  only open tasks. "Overdue" means the due date is before today in the device's time zone. Due dates are stored
  as `yyyy-MM-dd` strings so they don't shift between time zones. Screens refresh on significant time changes and
  when the app returns to the foreground.
- Deleting a list deletes its tasks. List names must be unique (case-insensitive) and not blank.
- A task moved to another list goes to the bottom of that list. A new task is added at the bottom of its list.
- Added a Medium priority that the data doesn't use, so the picker matches what people expect.
- Adding from Today defaults the due date to today. Adding from All defaults to the first list.

## How it was tested
- There is no tap automation here: no idb, and AppleScript hung waiting on a permission prompt. So I added
  test hooks driven by launch arguments: `-debugOpen today|all|list:<name>|edit:<id>|new:<name>`, and on a list,
  `-debugSteps toggle,collapse,move,swipe,bottom`. The steps call the same handlers the UI uses. `shot.sh`
  relaunches the app and takes a screenshot. I checked the reorder, the swipe delete, and persistence across
  relaunch by reading the saved JSON in the app container. I checked light and dark mode and AX-XXXL text size in
  screenshots. The finished app was reinstalled clean, so it starts from the 180 shipped tasks.
- Not verified by hand: the long-press drag gesture itself (I only tested the move handler it calls), the swipe
  gestures, and the pop-up menus. These use standard UIKit APIs.

## What was hard
- Combining reordering with a diffable data source. You have to subclass the data source for
  `canMoveRowAt`/`moveRowAt`, and re-apply the snapshot afterwards without a reload loop.
- Keeping the checkbox aligned with the first line of a wrapping title at every text size.
- At accessibility text sizes the editor's title field truncated, so I replaced it with a self-sizing
  `UITextView` that treats Return as "next" and turns pasted newlines into spaces.

## What would have made this faster
- A way to tap and swipe in the simulator, such as idb or `simctl io tap`, so gestures could be tested for real.
