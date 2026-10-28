# Tasks (UIKit): notes

## What's built
- **Lists screen**: Today and All, each with its open-task count, then "My Lists" with per-list open counts and colours.
  Tap + to create a list (the name must be non-empty and not already used, ignoring case). Swipe a list or long-press it
  to rename or delete it. Deleting asks first and says how many tasks go with it. Edit lets you reorder or delete lists.
- **List screen**: open tasks, then a collapsible "Completed n" section. Its collapsed state is remembered per list and
  starts collapsed. Tapping the circle completes or reopens a task, and so does a leading swipe. A trailing swipe
  deletes. Long-press and drag reorders open tasks, and so does "Reorder Tasks" in the ••• menu, which also has
  Show/Hide Completed, Rename List and Delete List. The title is in the list's colour.
- **Today**: open tasks due today or earlier, sorted by due date, plus completed ones in that range. **All**: every
  task. Both show which list each task is in. You can't drag-reorder there, since order only belongs to a real list.
- **Task editor** (a sheet; tap a task to open it, or "+" for a new one): title, notes, list (so a task can move to
  another list), an optional due date (switch plus inline calendar) and priority (None/Low/High). Add/Done stays
  disabled while the title is blank. Cancelling or swiping the sheet away with unsaved edits asks before discarding.
  Return in the title saves. A delete button appears when editing. New tasks from Today start due today.
- **Markers**: overdue dates are red, with a ! icon and "· Overdue". High priority shows an orange "!!!" before the
  title, low priority a grey "!" (as in Reminders). VoiceOver reads these too, and the row offers a custom
  Complete/Reopen action.
- **Persistence**: one JSON file in Application Support, written atomically after each change (bursts are grouped)
  and again when the app goes to the background. `Resources/tasks.json` (copied from `data/`) seeds it on the first
  launch only.
- Dark mode and accessibility text sizes were checked on the simulator. Rows wrap, the editor's List row stacks
  vertically, and the title field is a wrapping text view.

## Decisions and guesses
- A due date is a calendar day (`yyyy-MM-dd`) compared as a string against today's local date, so time zones can't
  move it by a day. "Overdue" means not done and due before today. A task due today isn't overdue.
- Today's count on the main screen covers open tasks only. On the Today screen, completed tasks in the same date
  range sit in its Completed section, so a task you tick off there doesn't just disappear.
- Each task's order is stored as an `order` integer. Reordering open tasks renumbers the list but leaves completed
  tasks where they were relative to each other.
- Lists get internal UUIDs, so renaming a list never touches its tasks.

## What was hard
- Drag-to-reorder with `UITableViewDiffableDataSource`: you have to subclass it for `canMoveRowAt`/`moveRowAt`,
  stop the store-change refresh from re-applying a snapshot in the middle of a move, then re-sync without animation.
  Drags are also kept out of the Completed section.
- Keeping the check circle lined up with the title's first line at every text size. A stack's first-baseline
  alignment doesn't work for an image-only button, so it uses a constraint tied to the font's line height.

## Testing, and what I couldn't check
- `simctl` can't tap or drag, so I added a testing launch argument (`-screen today|all|listN[,edit|new|expand|toggle|reorder]`)
  that opens a screen or editor directly. `shot.sh` relaunches with it and takes a screenshot. That's how I checked
  every screen, and that a completed task is still completed after a relaunch.
- I never ran the gestures themselves by hand: drag reorder, swipes, the alerts, the menus. The code follows the
  standard UIKit APIs, but those paths were built, not tested.

## What would have made this faster
- A way to tap and type on the simulator (an idb/XCUITest-style driver), so the gesture paths could be tested end to end.
