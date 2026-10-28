# Tasks (Exact): build notes

## What it is
- `app.contract` has the screens: Lists (Today, All, My Lists with open counts), a list's tasks
  (open tasks, then a Completed section that folds), a sheet to add or edit a task (title, list,
  optional due date, priority, notes, Delete), and a sheet to create or rename a list.
- `app.ts` keeps everything in one file (`app:/data/tasks-store.json`). On first launch it
  seeds that file from the bundled `data/tasks.json`, which it imports: 180 tasks in six lists,
  each list in the order its name first appears in the data. A list's order is its tasks' order
  in the file.
- Overdue dates show in red as "Overdue · Sat, Oct 17". High priority is a red `!!!`; low is a
  grey `!`. Due labels are Today, Tomorrow, Yesterday, a weekday name within the week, or else
  the date.
- To reorder, drag a task's grip (UIKit-style ≡); this works only in a real list, not in Today or
  All. Swipe a task to delete it. A list asks before it is deleted, by swipe, from its long-press
  menu or from the list screen's ••• menu, because deleting a list also deletes its tasks. A
  task deleted from its sheet asks first too.
- Whether Completed is shown or folded is a `persist` state, so it survives a relaunch. All
  data does too, and the relaunch test checks it.
- 10 tests in `app.test.contract`. `verify` (web) and `verify ios` both pass all of them.
  `verify ios` is clean: no findings in light or dark mode, nothing spills at the largest
  accessibility text size, and the real-touch scrolling check passes. `verify` on the web still
  lists a few web-only largest-text overflows in the web header and rows; it uses the browser's
  own layout, not UIKit's. The final build is installed on the simulator, and a launch on the
  real clock shows Today = 25.

## What was hard / what I had to look up
- **Drag to reorder.** It is not in any pattern. I found it in `contract-for-agents.md`:
  `reorderdrop` works only on `list virtualized=true`, and each row needs a `reorderFor` grip.
  A virtualized list can't be grouped (`appearance="auto"`), allows only one `for`, and wants
  one flow root per row. So the Completed header is a fake row in the same list, keyed
  `completed`. `delete=` had to sit on that flow root, not inside a component.
- **Testing the reorder.** `tap grip drag` works on the web but not in the iOS test runner;
  iOS needs `--touch platform`. On iOS I checked it by hand with real touches. The test uses the
  keyboard path instead (Space, ArrowUp, Space). That only works once the list has
  `reorderGroup`, which I found from a single phrase ("a grouped grip").
- **A button inside a `link` row.** Tapping the check circle also opened the edit sheet, so the
  circle now sits next to the link instead of inside it.
- **Route parameters.** In `add "/add/:scope"` the parameter is `params.scope`, not
  `params.id`. My mistake sent an empty list id, and the only symptom was a refusal in the reply.
- **Largest text size.** UIKit-style restacking needed `exactViewport().accessibilityTextSize`,
  which must be a root `resource` passed down as a prop. At those sizes the grip moves under
  the circle.
- **Tests on a virtualized list.** A row outside the rendered window needs
  `tap "tasks" into "completed"` before it can be tapped.

## Guesses
- I assumed the bundled data's "today" is the real date. The tests pin
  `epoch 2026-10-28T17:00:00Z`.
- Today includes the completed tasks that were due today or earlier, in its own Completed
  section. All is sorted by list, then by each list's own order.
- A task moved to another list in the edit sheet goes to the end of that list. A new task also
  goes to the end.

## What would have made this faster
- A pattern or recipe for a reorderable list with swipe-to-delete, a way to call a grouped list
  virtualized, and a sectioned (open/completed) layout.
- A compile warning when a `button` sits inside a `link` row, since both fire.
