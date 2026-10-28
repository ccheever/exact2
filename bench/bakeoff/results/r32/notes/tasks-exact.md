# Tasks (Exact): notes

Built from the `list-detail` pattern. The app is installed on simulator 486FD498…
as `dev.bakeoff.tasks.b979be4` ("Tasks"). `bun exact.mjs verify ios` is clean: 10 tests pass,
with no findings in light or dark mode or at the largest text size. Web `verify` also passes all
tests.

## What's there
- **Main screen** (grouped list, large title "Lists"): Today and All with their open-task counts,
  then My Lists with counts. The + button adds a list. A list row can be swiped to delete it
  (an alert asks first, since its tasks go with it), or long-pressed for Rename and Delete.
- **List screen** (a plain list, like Reminders): open tasks in their saved order, each with a
  circle to complete it and a grip to drag it. Swiping deletes a task. A "Completed · N" row has a
  Show/Hide button for the completed tasks below it. The bar's ⋯ menu has Rename List and
  Delete List; + adds a task.
- **Today** shows open tasks due today or overdue, sorted by date. **All** shows every open task.
  Both show each task's list name, and both have a collapsible Completed section. Neither can be
  reordered, because they have no single order of their own.
- **Add/Edit sheet:** title (Save stays disabled until there is one), a Due Date switch with a date
  picker, a Priority menu (None/Low/Medium/High), and notes. Edit also has Delete Task, which asks
  first.
- **Markers:** an overdue date is red with an alert icon ("Overdue · Oct 17"). High priority has an
  orange "High" badge and medium a grey "Medium" one. Dates show as Today, Tomorrow or "Nov 17".
- **Persistence:** everything goes to `app:/data/tasks-db.json`. On first launch that file is
  seeded from the bundled `data/tasks.json`, which `app.ts` imports, so it ships inside the app.

## What was hard
- **Drag to reorder.** Exact only allows `reorderdrop` on a `list virtualized=true`, and a grouped
  (sectioned) list may not be virtualized. So the list screen can't use a real section for
  "Completed". Instead it is one plain virtualized list built from a derived array of rows (open
  tasks, the Completed row, then completed tasks when shown). Every row has to be one component
  with a single root `row`: the compiler requires exactly one root per `for` item.
- `Entry` is a reserved shape name once `routes` exists, and `text` has no `subtitle` attribute.
  The compiler explained both clearly.
- **Sheets.** A `header` has to be the route's first child to become the navigation bar. My form
  components therefore take a `routeKey` prop and carry the route's `navigationKey` themselves.
- **Dates.** Contract can't parse dates, so a short days-from-civil function turns a `YYYY-MM-DD`
  due date into milliseconds for `formatDate`. "Today" comes from `exactTime()`, refreshed every
  minute.
- **Verify findings I fixed:**
  - Orange "!!!" text failed contrast, so it became a Badge.
  - Checkbox and grip touch targets were under 44pt.
  - A 4-segment priority control clipped at the largest text size, so it became a `select`
    (a pop-up menu).
  - Count rows on the main screen now wrap at large text.

## Guesses and choices
- The data has priorities none/low/high; I added Medium, as Reminders has it. Low gets no marker.
- New tasks go to the end of the list's open tasks.
- Today counts only open tasks due on or before the device's local date.
- Deleting a list deletes its tasks, after an alert that says how many.
- Swiping a task deletes it immediately, without asking (as Reminders and Mail do).

## Not done or limited
- Today and All can't be reordered, and tasks can't be added from them (only from a list).
- No search and no moving a task between lists. The spec asks for neither.
- The web build's wide (1280px) layout had "covered swipe action" findings. The web isn't the
  target, and the iPhone verify is clean.

## What would have made this faster
- A pattern or recipe for a reorderable list with a collapsible "completed" section, which is
  common in to-do apps. It would show the "virtualized, so no sections" workaround directly.
- `contract vocab reorder` returns nothing. The answer lives under `reorderdrop`/`reorderFor` and a
  paragraph of `contract-for-agents.md`.
- A date-parsing helper in Contract (ISO day to ms), or `formatDate` accepting an ISO day.
