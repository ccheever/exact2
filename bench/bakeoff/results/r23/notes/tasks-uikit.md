# Tasks (UIKit) — notes

## What's built
- **UIKit, programmatic.** `AppDelegate` + `SceneDelegate` (scene manifest in `app.env`). No SwiftUI.
- **Lists screen**: Today (open tasks due today or overdue) and All (all open tasks) with counts, then "My Lists" with open counts.
  Add List (toolbar); rename/delete by swipe, context menu, or a list's `…` menu. Deleting asks for confirmation and says how
  many tasks go with it. Duplicate names (case-insensitive) and empty names are rejected while you type. Edit mode reorders lists.
- **Task screen** (a list, Today, or All): open tasks, then a collapsible "Completed (n)" section. Whether it is collapsed
  is saved per screen. The checkbox completes or reopens a task, with haptics and a VoiceOver announcement. Swipe left deletes;
  swipe right completes or reopens. In a real list, long-press and drag to reorder, or use `…` → Select Tasks to Reorder.
  Moves stay inside their own section (open or completed). All groups open tasks by list; Today sorts by due date. Both show
  each task's list name in its colour.
- **Markers**: overdue = red date with an exclamation icon and "· Overdue" (open tasks only). High priority = red `!!!` plus a red
  checkbox ring. Low priority = grey `!`.
- **Editor** (modal sheet): growing title and notes fields (Return in the title moves to notes), Due Date switch with an
  inline calendar, and pop-up menus for priority and list (so you can move a task to another list). Add/Done is disabled while the
  title is blank. If there are unsaved changes, swiping the sheet down or tapping Cancel asks whether to discard them. Deleting from the editor asks first.
- **Persistence**: JSON in Application Support. `data/tasks.json` ships in the bundle as `Resources/tasks.json` and seeds the
  store on first launch. It is saved after every change and when the app goes to the background. An unreadable store file is moved aside, not overwritten.
- Dark mode and Dynamic Type up to AX5 were checked on the simulator. Cells reflow, and pop-up rows stack vertically at accessibility sizes.

## Decisions / guesses
- Due dates are stored as `yyyy-MM-dd` calendar days (not `Date`), so they never shift between time zones. "Overdue" means before today's local date.
- Lists are seeded in alphabetical order with fixed colours. New tasks go to the bottom of their list. The completed section uses the same
  manual order, so a task you reopen goes back to where it was.
- Today's Completed section holds done tasks that are due today or earlier.
- New Task from Today defaults to due today in the first list. From All it goes to the first list.

## What was hard
- Drag-to-reorder with `UITableViewDiffableDataSource`: you have to override `canMoveRowAt`/`moveRowAt` in a subclass, read the
  old snapshot *before* calling `super`, and keep the store-change notification from re-applying a snapshot in the middle of the move.
- Making rows re-render their attributed strings when Dynamic Type changes at runtime (the strings carry fixed fonts), and
  limiting how much the toolbar's "New Task" button grows.
- Testing without being able to tap: `simctl` cannot send touches. I added launch-argument hooks (`-route today|all|list:<name>|edit:<id>|new:<list>`,
  `-scrollToBottom YES`) and `shot.sh` to open screens and take screenshots. The data-layer flows (add, complete, reorder, rename, delete, relaunch) were checked with
  a temporary self-test route, which has since been removed. The touch gestures themselves (drag reorder, swipes, the checkbox tap, the alerts)
  were never exercised in the simulator. They are standard UIKit APIs, but they have not been tested by hand.

## What would have made it faster
- A way to send taps and drags to the simulator (idb or XCUITest support in the build script).
