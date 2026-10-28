# Tasks (Exact): notes

Installed on simulator EF198926-1958-489D-A7A4-9196E454DD3A as `dev.bakeoff.tasks.bc0c4b9` ("Tasks").

## What's there
- **Home ("Lists")**: Today (due today or overdue, not done) and All, each with its open count,
  then "My Lists" with each list's open count. + creates a list (sheet). A swipe deletes a list
  after an alert that says how many tasks go with it. A long press opens Rename / Delete.
- **A list's screen**: open tasks in the list's own order, then a "Completed · n" row that
  collapses or expands the done tasks (collapsed by default). The checkbox completes or reopens a
  task, with a success haptic. Tapping a task opens its edit sheet. + adds a task. Swipe deletes.
  A grip on each open task drags it to a new place. A task's long press offers Move Up, Move Down
  and Delete.
- **Today / All**: the same screen across lists. Today is sorted by due date, All by list. Each
  row also names its list. These views can't be reordered.
- **Task sheet**: title, notes, a Due Date switch with a date picker, priority (None/Low/Medium/High),
  and the list (so a task can move to another list). Delete asks first. Save stays disabled until
  there is a title.
- **Marks**: an overdue date is red, with an alert icon and "Overdue · Oct 17, 2026". Due dates
  read Today / Tomorrow / Yesterday when they apply. High priority shows an orange "High" badge.
- **Persistence**: everything is in `app:/data/store.json`. The first launch seeds it from the
  shipped `data/tasks.json`, which is bundled through `import` in app.ts: 180 tasks in 6 lists,
  sorted alphabetically. "Today" comes from the device clock and time zone.

## Verification
- `bun exact.mjs test web`: 8/8 pass.
- `verify ios`: no layout findings in light or dark mode, nothing spills at the largest
  accessibility text size, relaunch keeps the data. 7 of 8 tests pass. The one remaining finding is
  the web-only symbol warning. **The drag test fails on iOS**: the driver's real-touch drag
  (`tap grip-33 drag 0 -70`, and `--touch platform` with press/hold variants) never started a
  reorder on the iPhone build. No `reorderdrop` fired and nothing appeared in the journal. The
  same drag passes on the web, and the Apple host does have the code for it
  (`host/apple/Sources/ExactDrag/ReorderIOS.swift`). I could not tell whether a real finger
  works, so I added Move Up / Move Down to the task's long-press menu as a fallback that I
  confirmed on iOS. Treat drag-to-reorder on iPhone as **unverified**.

## What was hard / guessed
- **Reorder decided the screen's structure.** `reorderdrop` needs a `list virtualized=true`, and
  a grouped (inset) list can't be virtualized (`lower-grouped-list`). A virtualized list takes one
  keyed `for` with a single root per item, so it can't have sections. So the list screen is one
  plain virtualized list over computed rows: open tasks, then a "Completed" header row, then the
  done tasks when expanded. The swipe `delete=` sits on each row root, with `disabled` on
  non-task rows.
- Tests on iOS need `tap "tasks" into "<key>"` before touching rows outside the virtualized
  window. A relaunch on iOS returns to the home screen; the web keeps the URL. A long press on a
  list row inserted a moment earlier was reported as covered on iOS; it works after a relaunch,
  so the test long-presses an existing list.
- Child routes must carry their parent's parameters (`route-parent-param`), so the sheets are
  `/list/:id/task/:task` and `/list/:id/add`.
- Guesses: completed tasks start collapsed. Today's Completed section shows only tasks due
  today, so weeks of old overdue items don't bury it. A new task goes to the end of its list's
  open tasks. Seeded lists are alphabetical. I added a Medium priority, though the data has
  only none, low and high.
- The web build warns that `sf/list.bullet.circle.fill` is blank on web and Linux. That doesn't
  matter on iPhone.
- The disk filled up (ENOSPC) partway through and blocked every command for a while. It cleared
  on its own.

## What would have made it faster
- A recipe for "reorderable list with sections or a collapsible group". The guide doesn't say
  that reorder rules out grouped lists and sections until the compiler refuses it.
- A clear note on whether the iOS reorder grip can be driven by the agent and by `verify ios`,
  and if not, what to do instead.
