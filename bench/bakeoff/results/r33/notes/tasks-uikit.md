# Tasks (UIKit): notes

## What's there
- `Sources/App.swift`: the UIKit app and scene delegates (the scene manifest is in `EXTRA_PLIST`), plus launch-argument test hooks (below).
- `Sources/Model.swift`: the `Store`. It seeds from the bundled `tasks.json` on first launch and from then on saves the whole state as JSON in Application Support after every change, with an atomic write, so relaunching or killing the app loses nothing. Due dates stay `yyyy-MM-dd` strings and are compared against today's *local* date, so time zones never shift a day.
- `ListsViewController`: Today, All and the lists with their open-task counts.
  - New List is in the toolbar.
  - Swipe or long-press a list to rename or delete it. Deleting asks for confirmation and says how many tasks go with it.
  - Edit mode reorders lists, and tapping a list in edit mode renames it.
  - Duplicate list names are rejected, ignoring case.
- `TasksViewController`: one class for a list, Today and All, built on a diffable data source.
  - Open tasks come first, then a collapsible "Completed (n)" header. Whether it's collapsed is remembered per screen.
  - The checkbox, or a leading swipe, completes or reopens a task. A trailing swipe deletes it.
  - In a list, long-press and drag reorders the open tasks; "Edit Tasks" in the ••• menu shows reorder handles. The ••• menu also has Rename List and Delete List.
  - All groups open tasks by list.
  - Today shows open tasks due today or overdue. Its Completed section shows tasks due today, plus overdue tasks completed today, so ticking one off doesn't make it vanish.
- `TaskEditorViewController`: a sheet for adding or editing a task.
  - Title and notes are self-growing text views.
  - You can choose or change the list.
  - The due date is a switch with a calendar that opens when you tap the row.
  - Priority is None/Low/High, and an edited task can be deleted from here.
  - "Add" stays disabled until the title has text other than spaces.
  - If there are unsaved changes, cancelling or swiping the sheet down asks first.
- Marks: overdue dates show in red with an icon and the word "Overdue". High priority shows an orange bold `!!!`, low a grey `!`. VoiceOver reads these out too, and completing a task is a custom action.
- Checked in light and dark mode and at the largest accessibility text size. At that size, rows wrap instead of truncating, and the checkbox stops growing at a set size so titles keep room.

## How it was tested
I had no way to tap the simulator (no UI-automation tool), so I added launch-argument hooks:
- `-debugOpen <List name>|today|all[/new|/edit/<id>]` opens a screen or the editor.
- `-debugScrollBottom YES` scrolls the opened screen to the bottom.
- `-debugSelfTest YES` runs 15 checks on the store and prints PASS/FAIL to the console. They cover adding and ordering tasks, the Today rules, reordering, completing and reopening, deleting, renaming, editing, and deleting a list along with its tasks. All 15 passed.
- `shot.sh` relaunches the app and saves a screenshot to `shots/`.

To check persistence, I relaunched the app and read the JSON file in the app's container. Afterwards I uninstalled the app to clear the test data and reinstalled it fresh, so it is now on seed data.

## What was hard, and what I guessed
- **Crash found while testing:** setting a cell's `contentConfiguration` replaces its content view, so constraints pinned to the old view's margins guide crashed when the editor opened. The List row now uses the content configuration plus an invisible menu button covering the row.
- **Drag to reorder with a diffable data source:** I override `moveRowAt` in a `UITableViewDiffableDataSource` subclass, update the model and re-apply the snapshot on the next run-loop pass. Not tested with a real drag (no touch injection); the code follows the documented UIKit path.
- **Guesses about the spec:**
  - Lists start in alphabetical order.
  - A reopened task goes back to where it was.
  - Completed tasks are sorted newest-completed first.
  - "Today" means tasks due today or overdue.
  - Low priority also gets a subtle mark.

## Not done or not verified
- Swipes, drags and the alerts were never driven by real touches; only screenshots and the store self-test were checked.
- No undo for deletes, no search, no notifications.

## What would have made this faster
A way to tap, swipe and type in the simulator (for example `idb` or an XCUITest runner) would have let me test the touch interactions directly instead of through debug hooks.
