# Tasks (UIKit) — notes

## What's built
- `Sources/App.swift`: app delegate and scene delegate (the scene manifest is in `EXTRA_PLIST` in `app.env`).
- `Store.swift`: model, seeding from the bundled `Resources/tasks.json` on first launch, and JSON persistence in
  Application Support (saved after every change). Lists are seeded in alphabetical order. The order of the tasks
  array is the manual order within each list.
- `ListsViewController`: Today, All, and My Lists, each with its open-task count. Add List is in the toolbar.
  Swipe a list or long-press it to rename or delete it; Edit lets you reorder and delete lists. Deleting asks
  for confirmation and says how many tasks will go with the list. Names can't be empty or duplicate
  (the check ignores case).
- `TasksViewController`: open tasks, then a "Completed (n)" header that collapses the completed ones. Whether
  it's open or closed is saved per list. The checkbox completes or reopens a task, and so does a leading swipe.
  A trailing swipe deletes. You can drag to reorder (kept within the open or completed section), or reorder from
  Edit Tasks in the ⋯ menu, which also has Rename List and Delete List. Today shows open tasks that are due today
  or overdue, plus completed tasks due today. Today and All show which list each task belongs to.
- `TaskEditorViewController`: title, notes (grows as you type), list, priority (None/Low/Medium/High), and an
  optional due date with an inline calendar. Save is disabled while the title is empty. If you try to close it
  with unsaved changes, it asks whether to discard them. Editing a task also offers Delete.
- Markers: an overdue date is red with an icon and the word "Overdue", so color isn't the only cue. Priority
  shows as Reminders-style `!`/`!!`/`!!!`, orange for high. VoiceOver reads each task as one row, with
  complete/reopen as a custom action.

## Tested on the simulator
Screens checked in light mode, dark mode and the largest accessibility text size. Checked: reorder, complete,
delete, the collapsible completed section, and that everything survives a relaunch. These ran through a
temporary launch-argument hook, since there's no tap automation; the hook has been removed. Finally I
uninstalled and reinstalled so the app starts from the shipped data.

## Hard / guessed
- I couldn't tap through the UI. `simctl` only does screenshots and launches, so I drove screens and model
  operations through launch arguments and called the data source's move method directly. A real finger drag
  was never performed. It uses the standard drag/drop delegate with `moveRowAt` and a diffable data source,
  with snapshots re-applied on the next run-loop turn to avoid mutating the table inside its own callback.
- The spec only defines Today as "due today or overdue". I guessed that its completed section shows tasks
  completed with today's due date.
- The data only has none/low/high priorities. I added Medium as well.
- New tasks go at the bottom of the open list. New tasks from Today are due today and go in the first list.

## Would have made it faster
- A way to send taps and drags to the simulator (e.g. `simctl io tap`) or a preinstalled UI-test runner.
