# Tasks (Exact): notes

Built from the `list-detail` pattern. The app is installed on simulator
498D6D1F-C21C-4B31-90F4-5D857216E256 as `dev.bakeoff.tasks.ba3b653` ("Tasks").
`bun exact.mjs verify ios` is clean: 6 of 6 tests pass on iOS; no findings in light or dark mode; nothing spills at
the largest accessibility text size. `bun exact.mjs test web` also passes 6 of 6.

## What's there
- **Home:** Today and All with counts, then "My Lists" with each list's open count. The bar's + creates a list.
  Swiping a list deletes it, after an alert, because that takes its tasks too. A long press opens Rename and Delete.
- **A list:** open tasks in their order, each with a drag grip. Below them is a "Completed (n)" header that folds the
  completed tasks; the folded state persists. The circle completes or reopens a task, with a success haptic.
  Swiping deletes a task, and tapping it opens its edit sheet. The bar's ⋯ menu has Rename List and Delete List.
- **Add/edit sheet:** a grouped form with the title, a Due Date switch plus a date picker (so the due date is
  optional), a None/Low/Medium/High segmented control for priority, and notes. Editing adds a Delete Task button.
  Save stays disabled until there's a title, and the hint says why.
- **Marks:** an overdue date is red with an alert glyph and reads "Overdue · Sat, Oct 17". Priority shows as `!!!`
  in red for high, `!!` in orange for medium, and `!` in grey for low.
- **Today:** open tasks due today or earlier, by due date, with the name of each task's list. **All:** open tasks,
  one section per list.
- **Persistence:** `app.ts` keeps everything in `app:/data/db.json`. On first launch it seeds that file from the
  bundled `data/tasks.json` (all 180 tasks), building the six lists in the order they first appear. Task order is
  stored as an `order` field.

## What was hard / what I looked up
- **Drag to reorder.** In Exact this only works on a `list virtualized=true` with `reorderdrop` and a
  `reorderFor` grip. A virtualized list allows one keyed `for` with one root per row, and no sections. So the
  list screen is a single keyed list: open tasks, then a sentinel "Completed" row, then the completed tasks.
  The sentinel row is a `row` with `delete=` disabled, because `delete=` only works when the row is the `for`'s
  direct root; wrapping it in a `column` silently disabled swipe-to-delete, and the compiler only warned.
  I found this in `contract-for-agents.md` and `bench/list/exact-listbench`. It isn't in any pattern, and
  `contract vocab reorder` finds nothing.
- **No date parsing in Contract.** The data module formats the due-date labels (Today/Tomorrow/Yesterday/
  "Mon, Nov 2"). `loadDb(today)` takes today's date as an argument, because the data module can't read the clock.
- **Sheet headers.** For the bar to be the sheet's bar, the `header` has to be the route's first child, so the
  form components take a `navKey` prop and carry `navigationKey` on their own root.
- **Tests.** iOS relaunches at the home screen, while the web restores the route, so the relaunch test goes home
  first. A `link` row's `expect text` reads its children's joined text ("Garden17"), not its `aria-label`.
- The default agent epoch is 2026-01-01, so iOS screenshots taken without `--epoch` show Today as 0.

## Guesses
- I added a "medium" priority, though the data only has none, low and high.
- New tasks go to the end of their list. Dropping a task among the completed ones puts it at the end of the open tasks.
- Today and All have no add button. You add tasks from a list.

## Not finished / caveats
- On the web, `verify` reports items that I believe are web-only artifacts rather than app bugs: swipe-delete
  buttons "covered" at 1280 px, and text clipped at the largest size during the push transition. iOS is clean.
- `contract verify` reports engine-internal "DIVERGE" lines between its two semantics engines (runner vs lean), all
  "at boot" on a `delete` testId. They don't come from the app's behaviour, and every test passes.

## What would have made this faster
- A `reorder` entry in `contract vocab` (it says "not a name") that points to `reorderdrop`, or a pattern that has
  a reorderable list with swipe-to-delete and a collapsible section.
- A `parseDate` or day-arithmetic function in Contract for "YYYY-MM-DD" keys.
