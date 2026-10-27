# Tasks (UIKit): notes

## What's there
- `Sources/AppDelegate.swift`: UIKit app and scene delegates (the scene manifest is in `EXTRA_PLIST` in `app.env`), plus a test-only `-screen` launch argument.
- `Sources/Store.swift`: the model and JSON persistence (`Application Support/tasks-store.json`, written atomically after every change). On first launch it seeds from the bundled `Resources/tasks.json`, a copy of `data/tasks.json`.
- `ListsViewController`: Today and All, then My Lists, each with its open-task count. Lists can be added (toolbar), renamed and deleted (swipe, context menu, or the "…" menu on a list's screen; deleting asks first and says how many tasks go with it). Edit mode reorders lists. A new name can't be empty or match an existing list's name (case-insensitive).
- `TasksViewController`: one screen type for a list, Today and All, built on a diffable data source.
  - Open tasks come first, then a "Completed (n)" section you can collapse. The collapsed state is saved per list.
  - Tapping the checkbox shows the checkmark for 0.35 s, then the row animates into the other section; a second tap in that window cancels.
  - Leading swipe completes or reopens; trailing swipe deletes.
  - In a list, long-press and drag reorders tasks, and Edit mode shows reorder handles too. Tasks can only move within their own section (open or completed).
  - Today has Overdue and Today sections. Its completed section holds tasks due today, plus overdue tasks completed today, so finishing an overdue task doesn't make it vanish.
  - All groups open tasks by list.
- `TaskEditorViewController`: a sheet for adding or editing a task.
  - Fields: title (wraps; Return saves a new task), notes (grows as you type), a due-date switch with an inline calendar, priority (None/Low/High), list picker (moves the task), a Completed switch, and Delete.
  - Save stays disabled until the title has non-blank text. If there are unsaved changes, swiping the sheet down or tapping Cancel asks before discarding.
- Markers: overdue tasks show a red ⓘ and "Overdue · Fri, Oct 23". High priority shows a red "!!", low a grey "!". Due today shows in blue.
- System colors throughout. Checked in dark mode and at the AX5 text size; labels wrap.
- VoiceOver: each row reads as one element with complete/delete custom actions.

## What was hard or guessed
- I couldn't send taps or drags to the simulator; no tools were installed for it. So I added test-only launch arguments (`-screen list:Home|today|all|new:Home|edit:<id>`, `-actions toggle0,move0to2,delete1,collapse,add,edit,bottom`). They run the same code paths a touch would, and I screenshotted the results with `shot.sh`. The drag gesture and swipe actions use standard UIKit APIs but were **not checked with a real finger**.
- How a diffable data source behaves on a row move isn't well documented. After the store reorders, I re-apply the snapshot without animation, so the table ends up correct either way.
- Guesses:
  - "Today" counts open tasks due today or earlier.
  - Lists are seeded alphabetically with colors assigned in turn.
  - Completed sections start expanded.
  - New tasks go to the end of the list's open tasks, and the screen scrolls to them.
  - From Today or All, a new task goes into the first list (Today also sets the due date to today); the list can be changed in the editor.
- Due dates are stored as local `yyyy-MM-dd` strings and compared as strings, so there are no time-zone shifts. Screens refresh when the day changes.

## Not done
- No list colors or icons chosen by the user, no search, no reminders/notifications.
- Rotation is portrait-only, as the skeleton's Info.plist sets.

## What would have made this faster
- A way to send taps and drags to the simulator (e.g. idb or an XCUITest runner), so gestures could be tested for real.
- Crash reports in `DiagnosticReports` are all named `App-*.ips` across every bakeoff app, so finding mine took digging. `simctl launch --console-pty` was the quickest way to see the exception.
