# Tasks (UIKit) — notes

## What's built
- **Lists screen**: Today (open tasks due today or overdue) and All, with open counts, then "My Lists" with each list's open count. Add List (toolbar +), rename and delete through swipe actions, the context menu, or Edit mode (tap a row to rename; drag to reorder lists). Names can't be blank or duplicate (case-insensitive). Deleting a list that still has tasks asks first.
- **List screen**: open tasks, then a collapsible "Completed (n)" section. Tap the header to collapse or expand; the state is saved per list and also in the ⋯ menu. The checkbox (the whole leading strip of the row) completes or reopens a task. Drag a row to reorder open tasks; you can't drop into Completed. Swipe left to delete, right to complete or reopen. The ⋯ menu has Rename List and Delete List.
- **Today / All**: Today sorts by due date and shows each task's list name. All groups open tasks under headers in the list's colour. You can't reorder in these views.
- **Editor** (sheet): multi-line title, notes, a due-date switch with an inline calendar (tap the date row to show or hide it), and pull-down menus for priority (None / ! Low / !! Medium / !!! High) and list (so a task can move lists). Add stays disabled until the title has text. If there are unsaved changes, swipe-to-dismiss and Cancel ask before discarding. Delete Task appears when editing.
- **Markers**: overdue dates show a red ⓘ icon plus "Overdue · <date>" in bold. High priority shows a red "!!!" before the title. VoiceOver reads these out ("High priority", "Overdue, was due …"), and complete/reopen is a custom action.
- **Persistence**: on first launch the app seeds from the bundled `tasks.json`. After that, every change is written to `Application Support/TasksStore.json` (an atomic write). "Today" and overdue markers refresh on foreground and on significant time changes (such as midnight).
- Dark mode and Dynamic Type: system colours and text styles throughout. At accessibility sizes the editor's menu rows switch to a vertical stack.

## What was hard / guessed
- No way to tap in the simulator with the available tools. I added launch arguments as testing aids: `-debugScreen today|all|list:<id>|edit:<taskID>|new:<listID>` and `-debugScrollBottom YES`. I also ran the move, toggle, collapse and delete code paths once from a temporary in-app hook to check for crashes and for the new order persisting across relaunch. So the actual finger gestures (long-press drag, swipe, menus, alerts) were never exercised by hand.
- Getting drag-to-reorder working with a diffable data source: it needs a `UITableViewDiffableDataSource` subclass for canMove/moveRowAt, plus drag and drop delegates that limit drops to the open section.
- Guesses: seed lists are sorted alphabetically and given palette colours. Today has its own Completed section (done tasks due ≤ today). Data only has none/low/high priorities; I added Medium. New tasks go to the bottom of a list, and new tasks from Today default to due today. New tasks from Today/All go into the first list (you can change it in the editor).
- `UIBarButtonItem.Style.prominent` (iOS 26) is used for the Add/Done button.

## Not done
- Search, reminders/notifications, and undo for swipe-delete.

## What would have made this faster
- A simulator tap/gesture tool (or an XCUITest runner) to drive real interactions.
