# Tasks (Exact): notes

Bundle id `dev.bakeoff.tasks.b08593b`, name `Tasks`. It is built and installed on simulator
`EF198926-1958-489D-A7A4-9196E454DD3A`. The last `bun exact.mjs verify ios` was clean: 8 of 8 tests
passed, with no findings in light or dark mode and nothing spilling at the largest accessibility
text size. `bun exact.mjs test web` also passes all 8 tests.

## What's there
- **Home (Lists):** Today (open tasks due today or overdue) and All, each with its open count, then
  "My Lists" with each list's open count. The + button makes a new list in an alert with a name
  field; Create is disabled while the name is empty or already taken. Swiping a list, or Delete
  in its long-press menu, asks first and says how many tasks will go with it. Rename is in the
  long-press menu and in the list screen's ••• menu.
- **List screen:** open tasks come first, then a "Completed (n)" line with Show/Hide. Whether
  each list's completed section is open is remembered across launches. The circle completes or
  reopens a task (with a success haptic). The grip drags a task to a new place, and the order is
  saved. Swipe deletes a task. Tapping a task opens its edit sheet. Today and All show each task's
  list name and have no grips. Today is sorted by due date.
- **Add/Edit sheet:** a grouped form with title, notes, a Due Date switch with a date picker,
  priority (None/Low/High as a segmented control), and the list (a menu, so a task can move to
  another list). Save is disabled until there's a title. Edit adds a Delete Task button, which
  asks first. Adding from Today sets the due date to today.
- **Marks:** an overdue date is red and starts with "Overdue ·", so colour isn't the only signal.
  High priority shows a red "!!!" before the title, and VoiceOver reads "high priority" and the
  due date as part of the row.
- **Persistence:** the first launch seeds from `data/tasks.json`, which `app.ts` imports, so it is
  bundled. After that everything is saved in `app:/data/tasks-db.json`. A task's position in the
  stored array is its manual order.

## What was hard / looked up
- **Drag-to-reorder:** Exact only supports it (`reorderdrop` with a grip marked `reorderFor`) on a
  `list virtualized=true`, and the compiler won't allow a grouped list to be virtualized. So the
  task screens are a plain virtualized list, like Reminders. The rows are one array of
  task, "all done" and Completed-header items. Each row is a swipe-to-delete `row`, with
  `delete` disabled on the non-task rows. Learning this took several compile probes and a read of
  the host's `ReorderIOS.swift` tests.
- **Driving the app:** a drag, the header's pull-down menu and the context menus only behave like
  a person's touch under `agent ios --touch platform`. The authored tests therefore rename and
  delete through the long-press menu and the swipe, and I checked the header menu and the real
  drag by hand. After a relaunch, iOS opens on the home screen while the web restores the URL, so
  one test deep-links with `type "app" "/list/1"`.
- **Rows the test can't reach:** in a virtualized list, a row outside the visible window isn't in
  the tree. One test asserted a new row that was off-screen on iOS, so it checks counts instead.
- **Small things:** `Entry` is a name the router reserves. There's no `.replace` or `unwrapOr`, and
  a `derive` must fit on one line. Apple-only SF symbols need tamber icon tokens with SVG
  fallbacks for web and Linux; I drew small SVGs for the tray and list-bullet icons.
- **Guesses:** The data has only `none`, `low` and `high` priorities, so I offered those three, not
  Medium. Lists keep the order in which they first appear in the data. New tasks join the end of
  their list's open tasks. Completing a task keeps its place.

## Not done / limits
- The task list can't be searched, and there's no "Clear completed" button. The spec asked for
  neither.
- No authored test covers the drag, because the test language can't make a real touch. I drove it
  by hand on the simulator and it reordered correctly.
- On the web at 1280 wide, verify flagged list rows' delete buttons as covered by the split
  view's detail pane. iPhone only uses one column, and iOS is clean.

## What would have made this faster
- One line in the guide saying "drag-to-reorder needs `list virtualized=true`, not a grouped list,
  and that list can still swipe to delete". I found that out by trial.
- A pattern with reorderable rows (a to-do list) next to `list-detail`.
- Authored tests that could perform a reorder drop, the way a test can use a row's `-delete`.
