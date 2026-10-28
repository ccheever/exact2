# Tasks (UIKit): notes

## What's there
- `Sources/AppDelegate.swift` holds the app and scene delegates. The scene manifest is in `EXTRA_PLIST`.
- `Model.swift` holds `Store`, a JSON snapshot in Application Support. It is seeded from the bundled `tasks.json` on first launch and saved after every change. Lists are seeded in alphabetical order with a colour each. Due dates stay `yyyy-MM-dd` strings, so "overdue" and "today" are plain string comparisons against the local date.
- `ListsViewController` is the main screen: Today and All (open-task counts), then My Lists. You can add, rename and delete a list with a swipe, a context menu or Edit mode, and reorder lists in Edit mode. Duplicate or empty names are rejected live in the alert. Deleting a list asks for confirmation and gives its task count.
- `TaskListViewController` is one screen for a list, Today or All. It uses a diffable data source.
  - Open tasks come first, then a collapsible "Completed" section. Collapsed is the default, and the collapsed state is remembered per screen. The section has a Clear button.
  - Tapping the checkbox completes or reopens a task. Swiping left completes or reopens; swiping right deletes.
  - In a real list you can drag to reorder (long-press, or Edit mode), only within the same section. A reopened task goes back to its old place.
  - Today and All show each task's list name. Today shows open tasks due today or earlier.
- `TaskEditorViewController` is the add/edit sheet:
  - Fields: title, notes, an optional due date (switch plus a collapsible inline calendar), priority (None/Low/Medium/High) and list (moves the task).
  - Delete is offered when editing. You're asked to confirm before changes are discarded on Cancel or swipe-down.
- Marks: overdue dates are red with an icon and "· Overdue"; high priority shows an orange "!!!" before the title. VoiceOver reads both, and completion is a custom action.
- Dark mode and the accessibility text sizes were checked on screenshots. At large sizes, titles in the editor wrap (a text view rather than a field) and the form rows stack vertically.

## What was hard / guessed
- I couldn't drive taps on the simulator: `simctl` has no tap, and AppleScript/System Events timed out. So I checked the UI with screenshots, using development launch arguments that open screens directly: `-openScope <today|all|ListName>`, `-editTask <id|new|bottom|toggle:id>` and `-showCompleted YES`. `shot.sh` wraps them.
- I tested the store logic (move, toggle/reopen order, list create, rename and delete, persistence) with a temporary in-app self-test, which I've since removed. **Drag-reorder, swipes, menus and alerts were never exercised by a real touch.**
- Reordering with a diffable data source: the subclass's `moveRowAt` calls `super` so the snapshot records the move, then updates the model and re-applies.
- Guesses:
  - The Today screen's completed section shows done tasks due today or earlier.
  - "All" is one flat list (list order, then manual order) rather than grouped by list.
  - Completed sections start collapsed.

## What would have made it faster
- A way to send taps and drags to the simulator (an idb-style tool), or XCTest UI tests through the build script.
