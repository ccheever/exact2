# Tasks (Exact) — notes

Built and installed on simulator `C2188CC6-…` as `dev.bakeoff.tasks.b9e2706` ("Tasks").
`bun exact.mjs test web`: 9/9 pass. `verify ios`: 8/8 pass (the drag test runs on the web only, see
below), nothing spills at the largest text size, and the one finding left (the Completed row was
40 pt tall) is fixed in the installed build.

## What's there
- **Home:** Today and All, each with its open count, then My Lists, each with its open count.
  `+` makes a list. A long press on a list gives Rename and Delete; a swipe deletes, after an
  alert saying how many tasks go with it.
- **List screen:** a large title with an "N open" subtitle, `+` to add a task, and `…` with Rename
  and Delete List. The open tasks come first, then a "Completed · N" row that shows or hides the
  completed ones (the choice is saved). The circle completes or reopens a task, with a success
  haptic. Tapping a row opens the edit sheet. A swipe deletes. Drag the grip to reorder (real
  lists only; Today and All have no grips).
- **Add/edit sheet:** title, a Due Date switch with a date picker, Priority (None/Low/Medium/High),
  List, Notes, and Delete Task when editing. Save stays disabled until there's a title.
- **Marks:** an overdue open task shows its date in red with "· Overdue". A high-priority task
  has a red "!!!" before its title. Today and All also show each task's list name.
- **Persistence:** `app.ts` imports `data/tasks.json` and uses it to seed
  `app:/data/tasks-db.json` on the first launch. After that every write saves the whole file. The
  order of the tasks array is the manual order.

## What was hard / what I had to look up or guess
- **Drag to reorder** took the most digging. `contract vocab reorder` finds nothing. The answer was
  in `contract-for-agents.md`, the conformance fixture `host/web-js/conformance/reorder.contract`
  and the interaction-gallery app: `reorderdrop` only works on a `list virtualized=true` with a
  `reorderFor` grip. A virtualized list allows one keyed `for` with a single root per row, so the
  "Completed" toggle had to become a row in the same list. Rows are a `Row {key, kind, task}`
  union, and `TaskRow` draws either kind. A drop onto the Completed row or a completed task counts
  as "end of the open tasks".
- `tap "grip" drag to "row"` didn't trigger a reorder on the web, but `drag 0 -150` did. On iOS an
  authored test's drag is not a real touch, so the reorder test is `on web`. I checked the iOS
  drag by hand with `agent ios --touch platform`; it moved the task and the move persisted.
- **Today/overdue dates:** the contract can't parse dates. `app.ts` adds `dueMs` (UTC midnight),
  and the view formats it with `formatDay(dueMs, localNow, 0, "day")`, where `localNow` is wall
  time shifted to UTC. Overdue compares ISO strings with `<`. Agent drives default to 2026‑01‑01;
  `--epoch now` (or `epoch` in tests) is needed to see real overdue marks.
- Sheets made by a child component got the "header is not the route's first child" warning. The
  fix was to pass `routeKey` and put `navigationKey`/`navigationPresentation` on the component's
  root.
- `symbol:sf/...` is blank off Apple. I used roles instead: `circle` and `select-fill` for the
  checkbox, `menu` for the list icon.
- On the web, `verify` reports the hidden swipe-Delete buttons of Home rows as "covered" by the
  detail pane in the 1280-wide split layout. I accepted those in `app.json` `verify.accept` because
  iOS draws its own swipe actions.
- `contract verify` prints `DIVERGE` lines between its two semantics engines about synthetic
  `list-…-delete` testIds. That is the toolkit's own cross-check, not an app failure; tests and
  checks pass.

## Not done / judgment calls
- Deleting a task, whether by swipe or from the sheet, doesn't ask first (as in Reminders).
  Deleting a list does ask, because it takes the list's tasks with it.
- A large-text overdue line can wrap to two lines. I accepted that rather than truncating it.
- There's no search, and no way to reorder the lists themselves (the spec doesn't ask for either).

## What would have made this faster
- A `contract vocab reorder` (or `drag`) entry pointing to `reorderdrop`/`reorderFor`, and a
  `list-detail`-style pattern with a reorderable to-do list including a collapsible
  done section.
- A note that authored iOS test drags don't trigger reorders, and an `epoch` op for agent drives
  (the error message lists ops; the `--epoch` flag is in `--help`).
