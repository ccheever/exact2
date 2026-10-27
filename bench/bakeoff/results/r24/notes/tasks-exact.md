# Tasks (Exact): notes

Built from the `list-detail` pattern. `app.contract` is the view, `app.ts` the data module, which
seeds `app:/data/store.json` from the bundled `data/tasks.json` (imported, so it ships in the app)
the first time the app opens. After that the file is the source of truth. `app.test.contract` has
9 tests. `bun exact.mjs verify ios` is clean: tests pass, no findings in light or dark, nothing
spills at the largest text size, and data survives a relaunch. The app is installed on simulator
486FD498-….

## What it does
- **Home:** Today (due today or overdue, not done) and All (every open task), each with a count,
  then "My Lists" with each list's open count. Add a list with the + button. Touch and hold a
  list to rename or delete it, or swipe to delete. Deleting a list asks first, because its tasks
  go with it.
- **A list:** a plain list (as Reminders uses) of open tasks, each with a drag grip, then a
  "Completed · N" header that collapses or expands the done tasks. Which lists are collapsed is
  kept across launches. The checkbox completes or reopens a task (with a success haptic), tapping
  the title opens the edit sheet, and a swipe deletes. Swiping the Completed header clears that
  list's completed tasks after a confirmation. The ••• menu renames or deletes the list.
- **Add/edit sheet:** title, notes, a due-date switch with a date picker, priority
  (None/Low/Medium/High), and the list. A task moved to another list goes to the end of it.
  Edit also has Delete. Save stays disabled until there's a title. A list name that duplicates
  another (ignoring case) is refused with a message.
- **Marks:** an overdue date shows in red with an alert icon and the word "Overdue". High
  priority shows red `!!!`. Medium and low show grey `!!` and `!` (orange failed contrast).
- Today and All group tasks by list, in each list's own order.

## What was hard
- **Reorder vs. sections.** Drag-to-reorder (`reorderdrop`/`reorderFor`) only works on a
  `list virtualized=true`. A grouped (sectioned) list can't be virtualized, and a virtualized
  `for` must have exactly one root element. So the list screen is a single plain list, and the
  Completed header is a synthetic row (`Line` shape) inside it. That header can't turn off its
  swipe: `disabled` also makes the row inert, so its tap stopped working. I made its swipe mean
  "Clear completed", with a confirmation, which matches Reminders' Clear.
- **The checkbox's tap reached the row.** With `press` on the whole row, ticking the checkbox
  also opened the edit sheet. Now the press is on the title column only.
- **Rows inserted under a sheet were dead on iOS.** A grouped-list row added while the New List
  sheet covered Home stayed untappable (`inertBy=GroupedCell`) until a relaunch. Rebuilding the
  whole list didn't help: every cell built under the sheet came out inert. The workaround: the
  list sheet's mutation doesn't refresh. Instead it sets a flag, and an `after(450)` task
  refreshes Home once the sheet has dismissed. This looks like a host bug worth reporting.
  Editing a task from the Today/All grouped screens so that it changes section might hit the
  same bug. I didn't reproduce that or work around it.
- **Due-date labels.** Contract can't parse dates, so I wrote a small days-from-civil function
  that turns `YYYY-MM-DD` into UTC-midnight milliseconds for `formatDay` ("Today", "Tomorrow",
  "Oct 17, 2026").
- **Test quirks:** rows outside a virtualized list's window need `tap "tasks-list" into "<key>"`.
  A real-touch drag needs `agent ios --touch platform`. Agent drives default to 2026-01-01 unless
  you pass `--epoch`, so Today looked empty at first.

## Guesses
- Six lists, named in the order they first appear in the data. Their ids are `l1`…`l6`.
- New tasks go at the end of the open tasks. Priority "medium" is offered although the data only
  uses none/low/high.
- Today doesn't include completed tasks. All shows open tasks only.

## Not done / rough edges
- At the very largest text size a long word in a task title can be a little clipped next to the
  grip (`verify web` notes it; `verify ios` doesn't).
- The web build's 1280-wide layout has "covered" findings for the swipe buttons. I didn't polish
  it, since this is an iPhone app.

## What would have made it faster
- A recipe for "a reorderable list with a collapsible completed section" (a to-do list), or
  reorder support on grouped/sectioned lists.
- A way to switch `delete=` off on a row without `disabled` making it inert.
- Docs warning that a checkbox's tap inside a pressable row also fires the row's press.
