# Tasks (Exact): notes

Built from the `list-detail` pattern. `app.contract` is the view (home, list screen, task sheet,
list sheet); `app.ts` keeps everything in `app:/data/tasks-db.json`. On first launch it seeds from
`data/tasks.json`, which is imported, so it ships in the bundle. The six lists are sorted
alphabetically and the tasks keep their id order. The app is installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.tasks.b4c7602` ("Tasks").

State: 8 tests pass on web and iOS. `bun exact.mjs verify ios` is clean (no findings in light or
dark, nothing spills at AX5 text size, relaunch keeps the data). `verify` (web) still reports two
web-only findings at the largest text size (below).

## What it does
- **Home:** Today (open tasks due today or overdue) and All, each with its open count, then
  "My Lists" with open counts. `+` creates a list. A list row swipes to delete; deleting a list
  always asks first, because it also deletes the list's tasks. A long press opens Rename / Delete.
- **List screen:** open tasks, then a "Completed (n)" row that shows or hides the completed tasks.
  Show/hide is persisted. Each task has a checkbox (completing it plays a success haptic). A high
  task shows a red `!!!` (`!!` for medium, `!` for low). An overdue date reads "Overdue · Wed,
  Oct 21" in red. Today, Tomorrow and Yesterday are named. Drag the ≡ grip to reorder. Swipe a
  task left to Delete. The ⋯ menu renames or deletes the list. An empty list shows an empty state
  with "New Task".
- **Today / All:** the same screen without reordering. Each row also shows its list name. Today is
  sorted by due date; All follows the lists' order.
- **Task sheet** (tap a task, or `+`): title, notes, a Due Date switch with a date picker,
  Priority (None/Low/Medium/High) and List. Changing the list moves the task. Save stays disabled
  until there is a title. Editing a task adds Delete Task, which asks first.
- **List sheet:** name or rename. A duplicate name (case-insensitive) is refused with a message
  under the field.
- Everything is written to the file on every change, so it survives relaunch.

## What was hard
- **Reordering vs. a native grouped list.** `reorderdrop` only works on a `list
  virtualized=true`, and a grouped (`appearance="auto"`) list can't be virtualized. So the list
  screen is a plain virtualized list. Three consequences:
  1. The completed section can't be a real `section`. Instead it's a header row inside the same
     keyed `for`, over an array of `Line {key, header, task}`.
  2. `delete=` (UIKit's swipe) does nothing there, so the swipe uses the long form (a snap
     `scroll` with a red Delete button I paint myself).
  3. Each row needs exactly one flow root, so each task row is wrapped in a `column`.
- **Tapping a newly created list on iOS.** After creating a list through the sheet, the agent
  refuses to tap the new row: "GroupedCollectionView … covers its middle". Older rows work, and
  the new row works after a relaunch. None of these fixed it: removing the section footer,
  removing `delete=`/`contextmenu`, rebuilding the whole list with a keyed `for`, scrolling, or
  refreshing after the sheet closed. I couldn't tell whether a real finger is affected too: the
  agent's `--touch platform` mode runs the same check before it touches. The test reloads before
  tapping the new list, and that is the only workaround in it.
- **Reorder tests.** The drop position from `drag to "grip-41"` differs between web and iOS (just
  before vs. just after the target), so the test asserts that the task moved down, not its exact
  index. Keyboard reordering (Space + arrows) only works on grouped grips, so it doesn't apply here.

## Looked up or guessed
- Looked up: `reorderdrop`/`reorderFor` (contract-for-agents "virtualized"), the long-form swipe
  (`contract vocab swipe`), `edit-sheet`, `grouped-form`, `dates-and-days`, and the host's
  grouped-list hit test (GroupedListIOS.swift), to understand the tap refusal.
- **Today:** local midnight, taken as UTC ms (`floor(localNow/day)*day`). `app.ts` gives each due
  date as UTC midnight plus a short label, because the data module can't read the clock. Today's
  count covers open tasks due on or before today. Today's completed section shows done tasks due
  on or before today.
- **Priority:** the data only has none/low/high. I added Medium since Reminders has it.
- **List order** is alphabetical on first launch; new lists go at the end. New tasks go at the end
  of their list. A task moved to another list joins the end of it.
- Deleting a task by swipe doesn't ask (as in Reminders). Deleting from the task sheet asks, and
  deleting a list always asks.

## Not finished / known gaps
- Web `verify` at the largest text size still reports two findings:
  1. The "Due Date" label is clipped 12 px in its form row.
  2. After a drag, the page shows a 280 px horizontal overflow (probably the swipe scroll).
  iOS reports neither.
- Lists themselves can't be reordered (the spec didn't ask for it).
- The new-list tap issue above (possibly only in the agent).

## What would have made this faster
- A reorderable **grouped** list (or `delete=` and sections on a virtualized list). Today a
  Reminders-style screen forces you to choose between native rows and drag-to-reorder.
- A `list-reorder` pattern or recipe showing reorder plus swipe-delete plus a collapsible section.
- `--touch platform` (or a per-test equivalent) for authored tests, and a reorder drop position
  that is the same on web and iOS.
- Clearer diagnostics for "covers its middle" when the row is visibly on screen.
