# Tasks (Exact): notes

Built with Exact from the `list-detail` pattern. The bundle id is `dev.bakeoff.tasks.b616304` and the name is `Tasks`. It is installed on
simulator C2188CC6-…. The tests are `app.test.contract` (9 tests) and `tests/reorder.test.contract` (the drag test, run on the web).
`bun exact.mjs verify ios` is clean: the tests pass, and there are no findings in light or dark mode or at the largest text size, plus a
relaunch check.

## How it works
- `app.ts` imports `data/tasks.json` and seeds `app:/data/store.json` from it on first launch. The store is the lists, the tasks
  with an `order` field, and the next id. From then on that file holds everything, so everything survives a relaunch.
- The main screen has Today and All with their open counts, then "My Lists" with each list's open count. Swiping a list or
  long-pressing it opens Delete or Rename. Deleting a list asks first, because it deletes the list's tasks too.
- A list's screen shows open tasks in order, then an "N Completed" row that shows or hides the completed tasks. The circle completes a
  task (with a success haptic) or reopens it. Tapping the rest of the row opens the task sheet. The grip drags to reorder (real lists
  only, not Today or All). Swipe deletes. The "…" in the bar holds Rename and Delete for the list.
- The task sheet has a title, notes, a due-date switch with a date picker, a segmented priority control, and a list picker
  (so a task can move to another list). The edit sheet also has Delete Task, which asks first.
- A past due date shows in red with an alert icon as "Overdue · Oct 17". Priority shows as `!!!`, `!!` or `!`; high is red.
  "Today" means due today or overdue. Dates use the device's own time zone, updated every minute.

## What was hard or had to be guessed
- **Reorder** needs `reorderdrop` on a `list virtualized=true`, and the compiler refuses virtualized *grouped* lists. So a
  list's screen is a plain list (as Reminders' is). A virtualized list allows one row root per `for`, so the "Completed" header
  and the "All done" line are sentinel rows in the same array as the tasks. I found this out from compiler errors and conformance
  fixtures (`host/web-js/conformance/reorder.*`), not from a recipe. There is no `reorder` recipe; `contract vocab reorder` finds nothing.
- **iOS drag in tests.** The agent's drag on iOS only works with `--touch platform`, and that flag then breaks the swipe-delete
  and popover-menu taps in the other tests. So the drag test runs on the web. On the simulator I checked the drag by hand with
  `agent ios --touch platform "tap grip-40 drag 0 -150 over 600 hold 400"`, and the order persisted. On the web the drag also
  needs a `hold`; `drag to <node>` alone dropped nothing.
- **A button inside a `link` row**: tapping the circle also opened the task (the press reached the link too). The fix was a
  row holding the button *beside* a `link`.
- A route-param slip of mine (`e.params.id` on the `/add/:list` route) went unflagged and left the sheet's list empty. A
  warning for reading a param the route doesn't declare would have caught it.
- `-exact-accent` isn't a colour role; the compiler suggested `AccentColor`. Orange "!!!" failed the contrast check on white,
  so high priority is red.

## Not done / limitations
- The drag test is not part of `verify ios` (see above). Reorder is limited to a list's open tasks. Completed tasks keep their
  order, and Today and All don't reorder.
- I didn't add search or reminders/notifications; the spec doesn't ask for them.

## What would have made this faster
- A `reorder` recipe: "a sortable list = `list virtualized=true reorderdrop=…` + `box reorderFor=…` grip, not grouped, how to
  put a section header among its rows, how to test it on iOS".
- A test step that does a real-touch drag on iOS without switching the whole run to `--touch platform`.
