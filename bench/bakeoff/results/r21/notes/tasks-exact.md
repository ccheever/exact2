# Tasks (Exact): notes

## State
All seven spec features are built and work on the iPhone simulator (CEDFB9C9-…). The app is installed there as
`dev.bakeoff.tasks.4efbe7`, named "Tasks". `bun exact.mjs verify` (web) and `verify ios` both pass the 6 authored tests
(`app.test.contract`). These cover counts, Today/All, the completed section folding, the checkbox, add/edit with due
date, priority and notes, swipe delete, delete from the sheet, list create/rename/delete with a duplicate-name refusal,
drag reorder, and relaunch. The iOS check found nothing in light or dark mode.

- Main screen: Today (due today or overdue, open) and All, with counts, then "My Lists" with each list's open count.
  Swipe a list to delete it (an alert asks first, because its tasks go with it). Touch and hold a list to rename or delete it.
  "+" makes a new list.
- List screen: open tasks, then "Completed · N" with Show/Hide (the choice persists). The circle completes or reopens a
  task. Tap a task to edit it in a sheet. Drag a task by its grip to reorder it. Swipe a task to delete it. "…" has
  Rename List, Show/Hide Completed and Delete List. "+" adds a task.
- Overdue dates are red with "· Overdue". Priority shows as `!`, `!!` or `!!!`, and high is red.
- Data: `app.ts` imports `data/tasks.json` and seeds `app:/data/tasks-store.json` on first launch. Every change is
  written there, so it survives relaunch.

## What was hard / guessed
- **The bundle id.** Exact's `app.schema.json` rejects `dev.bakeoff.tasks.4efbe7`, because its segments must start with a
  letter (Apple allows a digit). So `app.json`/`app.ts` use `dev.bakeoff.tasks.x4efbe7` for building. At the end I copied
  the built `Tasks.app` to `.exact/final/` and changed `CFBundleIdentifier` (and `receipt.json`) to
  `dev.bakeoff.tasks.4efbe7`. I re-signed it ad hoc, uninstalled the `x4efbe7` build and installed this one. It launches
  and shows the data. Exact's own `agent ios`/`verify ios` drive only the `x4efbe7` id. A rebuild reinstalls under that
  id, so the re-stamp has to be repeated after any rebuild.
- **Reordering.** Dragging needs `reorderdrop` on a `list virtualized=true`, and a virtualized list can't be a grouped
  (`inset-grouped`) list. So task screens are plain virtualized lists with rows I drew myself (separators, checkbox,
  grip). They hold one keyed `for`, so the Completed header and All's per-list headings are rows in the same array (the
  `Line` shape). Native swipe-to-delete (`delete=`) still works on those rows on iOS.
- Sheet headers had to be the route's first child to become the navigation bar. So the sheet components take `navKey`
  and carry the route's `navigationKey` themselves.
- Due-date labels: the data module returns `dueMs`, which is UTC midnight of the date. The view calls `formatDay(dueMs,
  now + utcOffset, 0, "day")` to get Today, Tomorrow or "Nov 3, 2026" in the person's own day.
- "Today" means open tasks due today or earlier. Its Completed section shows done tasks due by today.

## Not finished / known
- At the largest accessibility text size, one long word ("tomatoes") next to a priority mark is 13 pt wider than its
  row. That is the one remaining `verify ios` finding.
- On iOS, the My Lists chevrons draw darker than the Today/All ones, even with a tertiary colour set.
- The web build's wide-window and largest-text checks still report some clipping. The app is iPhone-only.

## What would have made it faster
- A schema that accepts any valid Apple bundle id, or an override for it.
- A reorderable grouped list (or drag on `inset-grouped` sections), and a "collapsible section" idiom in the docs.
