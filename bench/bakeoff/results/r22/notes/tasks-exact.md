# Tasks (Exact): notes

Built from the `list-detail` pattern. `app.contract` is the view, `app.ts` the data module and
`app.test.contract` holds 7 tests. `bun exact.mjs verify ios` is clean: the tests pass, and it
finds nothing in light, dark, the largest text size or after a relaunch. The app is installed on
simulator 498D6D1F… as `dev.bakeoff.tasks.b4b58e6`.

## How it works
- **Data:** `app.ts` imports `data/tasks.json`. At first launch it makes six lists from it
  (alphabetical, ids `l1`…`l6`) and keeps each task's position within its list. After that,
  everything lives in `app:/data/store.json`.
- **Home:** Today and All, then My Lists, each with its count of open tasks. You can swipe a list
  to delete it, which asks first and says how many tasks go with it. A long press gives Rename
  and Delete, and + makes a new list. Duplicate names are refused with a message.
- **List screen:** the open tasks, then a "Completed · N" header that shows or hides the done
  ones. The circle completes a task or reopens it (with a success haptic). Tapping a row opens
  the edit sheet, swiping deletes, and the grip drags to reorder. The ⋯ menu renames or deletes
  the list. Today (due today or overdue, oldest first) and All show each task's list name and
  have no grips.
- **Add/edit sheet:** title, notes, a Due Date switch with a date picker, priority
  (None/Low/Medium/High) and list (so a task can move to another list). Done stays disabled
  until there is a title, and the sheet has Delete.
- **Markers:** an overdue date is red, as "Overdue · Fri, Oct 23, 2026". Today, Tomorrow and
  Yesterday are shown as words. High priority is a red "!!!"; medium and low are grey "!!" and
  "!".
- **Today's date:** comes from `exactTime()` and a 30-second tick, because the data module may
  not read the clock.

## What was hard
- **Drag to reorder:** this only works in a `list virtualized=true`, and Exact refuses that on
  a grouped (`appearance="auto"`) list. So the task list is a plain virtualized list whose rows,
  separators, Completed header (a row inside the list) and grip I drew by hand. It took some
  digging in `contract-for-agents.md` and two apps in the exact2 checkout (`interaction-gallery`,
  `bench/list`) to find `reorderdrop` and `reorderFor`.
- **An apparent Exact/iOS bug:** after any sheet presented over the home screen closed (Cancel or
  Save), its grouped list stopped taking taps, both from the driver and from real
  (`--touch platform`) touches. Pushed screens and alerts did not cause it. My workaround
  rebuilds the home list when such a sheet closes (`homeGen` in `app.contract`). I could not
  confirm that swiping a sheet down also goes through `back`. If it doesn't, the home list could
  still go dead after that kind of dismissal.
- **iOS and web disagree in places:** the same drag distance moved a row a different number of
  places, and a reload on iOS returns to the home screen. I made the tests independent of both.

## Guesses
- Priorities: the data has none/low/high; I added Medium as Reminders does.
- The lists are ordered alphabetically, and a new list goes at the end.
- Deleting a list asks first, but deleting a task does not (as in Reminders).
- A task dropped among the completed tasks lands after the last open one.

## Not done
- Lists can't be reordered.
- The 1280-wide web layout has "covered" findings (swipe buttons in the split view). I left
  them because this is an iPhone app; `verify ios` is clean.

## What would have made this faster
- A `reorder` entry in `contract vocab`, or a recipe for a reorderable grouped list. A search for
  "reorder" only found the symbol.
- An `epoch` drive step for `agent ios`; it is the `--epoch` flag instead.
