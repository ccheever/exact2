# Tasks (Exact) — build notes

## What was built
- **Lists screen**: Today (open tasks due today or overdue) and All with open counts, then
  "My Lists" with each list's open count. + makes a list. Swiping a list deletes it after an
  alert (it takes its tasks with it); long-pressing a list opens Rename / Delete.
- **List screen**: open tasks in the list's own order, then a "Completed (n)" header row that
  collapses or expands the completed tasks. Which screens are collapsed is kept with `persist`.
  The circle completes or reopens a task (with a success haptic). Tapping the row opens the
  edit sheet. Swipe deletes. The grip on the right drags a task to a new place. The ••• menu
  renames or deletes the list. Today and All show the same sections with each task's list name;
  Today is sorted by due date. Neither has a grip, because the order only means something
  inside one list.
- **Add / edit sheet** (grouped form): title (Return saves), notes, a Due Date switch that shows
  a date picker, a None/Low/High segmented priority, and a List picker (so a task can move
  between lists). The edit sheet also has Delete Task. Save stays disabled until there's a title.
- **Marks**: an overdue date is red with an alert icon ("Overdue · Oct 18"). High priority is
  a red "!!!" before the title, low a grey "!". Dates read Today / Tomorrow / Yesterday / a
  weekday within the week / "Oct 18".
- **Persistence**: everything goes to one JSON file (`app:/data/tasks-state.json`). On first
  launch it is seeded from `data/tasks.json`, which `app.ts` imports and so ships inside the
  bundle. Lists are seeded alphabetically.
- 8 contract tests: counts, complete/reopen, collapse plus relaunch, add/edit with
  date/priority/notes, swipe delete plus relaunch, list create/rename/duplicate
  refusal/delete, Today, and drag reorder plus relaunch.

## What was hard
- **Reorder vs. sections.** In Exact, `reorderdrop` only works on a `list virtualized=true`
  with exactly one keyed `for` and one root element per row. A virtualized list can't be
  grouped (inset-grouped) or have `section`s. So the "completed" section is a header row
  inside the same flat list (rows = open tasks + header + completed tasks). The header row
  is a pressable `row` whose `delete=` is disabled.
- **A checkbox inside a tappable row.** When the row was a `link`, tapping the inner circle
  button opened the edit sheet instead of toggling the task. Making the row a `row press=…`
  fixed it: the inner button's press wins and the row's press opens the sheet.
- Rows hidden off-screen in a virtualized list can't be tapped in tests ("hidden or inert"),
  so the collapse test uses a short new list.
- **A row added while a sheet was still open couldn't be tapped on iOS.** At first the
  new-list sheet stayed up until the write's reply came back (so it could show a "name taken"
  refusal). The new row appeared in the grouped list but the agent couldn't tap it (UIKit's
  collection view "covered its middle") until the app was relaunched. Rebuilding the whole list
  didn't help. The fix: the sheet checks the name against the other lists itself (Done is
  disabled and the reason shows in red), closes, and only then sends the write. The rows then
  arrive after the sheet has gone. This looks like an Exact/UIKit host issue worth reporting.
- The 44 pt tap target for the checkbox: negative margins to keep it visually tight were
  flagged as overflow, so the row's padding was trimmed instead.
- On iOS a relaunch opens at the root (the web restores the open screen), so the relaunch tests
  check the stored state, not the screen.
- The `header` of a sheet has to be the route's first child. Otherwise it isn't the
  navigation bar, so the form components take a `navKey` and carry `navigationKey` themselves.

## Looked up / guessed
- Looked up: reorder (`reorderdrop`/`reorderFor`, found by grepping docs since
  `contract vocab reorder` had no entry), the edit-sheet recipe, dates-and-days (the data
  module can't read the clock; `today` comes from `exactTime()` and is passed to `loadDb`),
  choice (segmented control), menu, swipe.
- Guessed: importing `./data/tasks.json` from `app.ts` (no pattern did it; it worked).
  Defaults: the completed section starts expanded; a new task goes to the end of its list's
  open tasks; a task moved to another list goes to the end of that list.
- Without `--epoch`, the agent's virtual clock starts at 2026‑01‑01, so Today showed 0 in the
  first screenshots. The real app uses the device clock.

## Status
`bun exact.mjs test web`: 8/8. `verify ios`: 8/8 tests, no findings in light or dark mode, nothing
spills at the largest accessibility text size. The only remaining items are the two compile
warnings below. The app is built and installed on simulator EEFC0029-… (`bun exact.mjs ios`).

## Not finished / known gaps
- The SF Symbols for All (`tray.fill`) and lists (`list.bullet.circle.fill`) are blank on
  the web host (warnings only; this is an iPhone app).
- No search, and no medium priority (the data only has none/low/high).

## What would have made this faster
- A pattern or recipe for a "reorderable list with a collapsible completed group". This is the
  standard to-do layout, and finding out that reorder rules out sections took several build
  round trips.
- Saying in the docs that a `button` inside a `link` row doesn't win the tap.
- `contract vocab reorder` pointing at `reorderdrop`.
