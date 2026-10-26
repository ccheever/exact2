# Tasks (UIKit): notes

## What's there
- **Lists screen**: Today and All, then "My Lists", each with its count of open tasks. Add List (toolbar), rename and delete
  (swipe, long-press menu, or Edit mode; in Edit mode, tapping a list renames it), and lists can be reordered in Edit mode.
  New and renamed list names are trimmed, and empty or duplicate names (ignoring case) are rejected inline in the alert.
  Deleting a list asks first and says how many tasks go with it.
- **Task screen** (a list, Today or All): open tasks, then a collapsible "Completed (n)" section. Its collapsed state is
  remembered for each screen. The checkbox completes or reopens a task, and so do a leading swipe and a VoiceOver custom
  action. Trailing swipe deletes. In a real list you reorder by long-press dragging, or with Edit mode's reorder
  controls, and a task can only move within its own section. Today and All show which list each task is in and
  can't be reordered (their order is computed: by due date, or by list). A list's ⋯ menu offers Rename and Delete.
- **Editor** (sheet): title (wraps; Return ends editing), list, priority (None/Low/Medium/High), an optional due date
  (switch plus an inline calendar that opens or closes when you tap the row), notes, and Delete. Save is disabled until
  there's a title. Swiping the sheet down or tapping Cancel with unsaved changes asks before discarding.
- **Marks**: an overdue due date is red with an alert icon and "· Overdue". Priority shows as red **!!!** for high,
  orange !! for medium and grey ! for low, Reminders-style.
- **Persistence**: JSON in Application Support, written atomically after every change. On first launch the store is
  seeded from the bundled `tasks.json` (it's in `Resources/`).
- Dark mode, Dynamic Type up to AX5 (menu rows stack vertically at accessibility sizes), and VoiceOver labels.

## Decisions and guesses
- Today = open tasks due today or earlier. Its Completed section only shows tasks *due today*, not every completed task
  with a past date. A finished task isn't overdue.
- Due dates are kept as `yyyy-MM-dd` strings, so they never shift with the time zone. Today and the overdue marks
  refresh when the app returns to the foreground and when the day changes (significant time change).
- The data only has the priorities none/low/high. I added Medium because Reminders has it.
- Seed lists are sorted alphabetically. List colours are assigned automatically.
- New Task from Today or All goes to the first list (from Today, it's due today). You can change the list in the form.
- When you reopen a task, it goes back to its old place among the open tasks.

## Testing
I have no way to tap in the simulator, so I added launch arguments: `-screen today|all|list:<name>|edit:<id>|new:<name>`
opens a screen, and `-selftest YES` (see `Sources/SelfTest.swift`) drives the real data source's move and delete paths
and the store. All 15 checks passed. After a relaunch, a rename and a reorder were still there, and the file on disk
matched. I checked each screen with screenshots in light, dark and AX5. The data was reset afterwards (uninstall and
reinstall). Real finger dragging and swipes were **not** tested by hand. They use the standard
UITableView drag/drop with diffable `moveRowAt` and swipe-action APIs.

## What was hard / would have helped
- With no way to send taps, gestures (drag reorder, swipes, menus) can only be checked by calling the code directly.
  A tap/drag tool for the simulator would have made this faster and more reliable.
- Reordering with a diffable data source needs a subclass and has to update the snapshot by hand before persisting.
- A careless `sed` removed every trailing comma from one file and I had to repair it. Editing with a script that
  asserts each replacement is safer.
