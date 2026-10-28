# Tasks (Exact): notes

Built from the `list-detail` pattern. Files: `app.contract` (view), `app.ts` (data),
`app.test.contract` (11 tests). `bun exact.mjs test web` passes 11/11 and `bun exact.mjs verify ios`
is clean: no findings in light or dark, nothing spills at the largest text size, and the app
shows its saved data after a relaunch. The app is installed on simulator 498D6D1F-….

## What it does
- **Home:** Today (open tasks due today or overdue) and All, each with its open count, then "My
  Lists" with each list's open count. The + in the bar adds a list.
- **List screen:** a plain list like Reminders'. Open tasks come first, then a Completed row
  (with its count) that shows or hides the done tasks. The setting is kept across launches.
  Tapping the circle completes a task (with a success haptic) or reopens it. Tapping the row
  opens the edit sheet. Swipe deletes. The grip at the trailing edge drags the task to a new
  place in the list.
- **Today and All** use the same screen and show each task's list name. Today sorts by due date,
  and + there makes a task due today.
- **Add/edit sheet:** title, notes, a Due Date switch with a date picker, priority
  (None/Low/Medium/High), and a list picker, so a task can move to another list. Save stays
  disabled until there is a title. In the edit sheet, Delete Task asks first.
- **Markings:** overdue tasks show a red alert icon and "Overdue · Sat, Oct 17" in red. Priority
  shows as `!!!` in red for high, `!!` in orange for medium and `!` in grey for low.
- **Lists:** create, rename and delete from the list screen's ⋯ menu, or by long-pressing a list
  on the home screen. Rename and new list refuse an empty name or one that's already used.
  Delete asks first and says how many tasks go with the list.
- **Persistence:** `app:/data/tasks-store.json`. On first launch it's seeded from the shipped
  `data/tasks.json`, which `app.ts` imports so it's bundled. The six lists keep the order in
  which they first appear in the data.

## What was hard / what I had to look up
- **Drag to reorder** is only on a `list virtualized=true` (`reorderdrop`, plus `reorderFor` on a
  grip), and a grouped (inset) list can't be virtualized. That forced a plain list for the
  tasks, which also needs exactly one root element per row. So the Completed toggle is a row
  inside the same list rather than a section header, and the drop handler treats a drop on
  the Completed row or a done task as "end of the open tasks". The docs for this are in
  `contract-for-agents.md`, not in a recipe, and `contract vocab reorder` finds nothing.
- **Driver quirks on iOS** that are not app bugs, but each cost time:
  - `tap X drag to Y` reordered on the web but not on iOS. A real-touch drag of 150 pt does work.
  - Under the real-touch runner (which any test with a drag turns on), `tap <grouped row>
    contextmenu` aims about 134 pt above the row, so it long-presses the wrong place. The
    long-press menu works in a normal drive, but the tests reach those actions through the
    list's ⋯ menu instead.
  - When home-list rows had `delete=` (swipe to delete), the driver refused every tap on them
    after any sheet closed ("GroupedCollectionView covers its middle"). The simulator is
    headless, so I couldn't check a real finger. To be safe, I removed swipe-to-delete from
    *list* rows; lists delete from the long-press and ⋯ menus. Task rows keep swipe-to-delete,
    which behaved correctly.
- **The date:** `app.ts` can't read the clock, so the view passes today
  (`exactTime()` + a one-minute tick) to `loadTasks(today)`. `app.ts` then works out the
  overdue flag and the due labels (Today / Tomorrow / weekday / "Tue, Nov 17"). Agent drives
  default to 2026-01-01, so Today showed 0 until I passed `--epoch now`.

## Guesses
- I added Medium priority as an option, though the data only has none, low and high.
- "Today" means open tasks due on or before today. Its Completed section shows only tasks due
  today, not every past completion.
- New tasks go at the end of the list's open tasks. Moving a task to another list puts it at
  the end of that list's open tasks.
- The Completed section starts expanded.

## Not done
- No search, no due times or reminders, no list colours or icons.

## What would have made this faster
- A recipe for "a reorderable list of tasks with a completed section", or a `contract vocab
  reorder` entry pointing to the virtualized-list rules.
- Driver fixes for the long-press offset under real touch and the post-sheet hit test on
  swipeable grouped rows, or a way to send a raw coordinate tap to check a real finger.
