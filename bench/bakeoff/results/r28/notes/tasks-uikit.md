# Tasks (UIKit) — notes

## What's there
- `Sources/App.swift`: app and scene delegates, all in code with no storyboards. The scene manifest is in `EXTRA_PLIST`. There is also a small `-open …` launch-argument hook I used to put the simulator on a given screen for screenshots.
- `Model.swift`: the `Store`. On first launch it seeds from the bundled `tasks.json` (copied into `Resources/`). It saves everything to `Application Support/store.json` after every change: lists, tasks, task order and which Completed sections are collapsed.
- `ListsViewController`: shows Today (open tasks due today or overdue) and All, then My Lists, each with its open-task count. You can create a list from the toolbar. Rename and delete work by swipe, context menu or Edit mode. Deleting asks for confirmation and says how many tasks will go. Duplicate and empty list names are rejected inside the alert. Lists can be reordered in Edit mode.
- `TasksViewController`: uses a diffable data source, so completing or reopening a task animates it between sections. Open tasks come first, then a "Completed (n)" section you can collapse with a tap.
  - Long-press and drag reorders tasks within a list; drops into another section are refused. The ••• menu also has "Reorder Tasks" for edit-mode handles.
  - Swipe left to delete, swipe right to complete or reopen.
  - Today splits into Overdue and Today sections. All groups by list.
  - The list's own ••• menu can rename or delete it.
- `TaskEditorViewController`: a modal form with:
  - Title and a notes field that grows as you type.
  - A list picker, so a task can be moved to another list.
  - A due-date switch with an inline calendar, and priority None/Low/High.
  - Delete, when editing an existing task.
  - Save stays disabled until the title has text and something changed. Cancelling or swiping the sheet down asks before throwing away edits.
- Markings:
  - Overdue (open, due before today): red, with an icon and the word "Overdue", so it isn't shown by colour alone.
  - High priority: a red "!!!" before the title; low priority gets a grey "!".
  - VoiceOver reads these out, and each row has a Complete/Reopen custom action.

## Guessed or decided
- The data only has the priorities none, low and high, so the app offers just those three.
- "Today" uses the device's current day and refreshes when the day changes. Completed tasks are never marked overdue.
- The six lists are sorted alphabetically when seeded and get colours automatically. New tasks go to the bottom of their list.
- Completed sections start expanded, and the collapsed state is saved per screen.
- Reordering only works inside a real list. Today and All are sorted, not ordered by hand.

## Hard / not verified
- `simctl` can't tap, drag or swipe, so I checked rendering through launch-argument screenshots: light, dark and AX5 text size, plus the editor. I checked persistence by inspecting `store.json` in the app container. I could not run drag-to-reorder, swipe-to-delete, the collapse toggle or the alerts on the simulator. They are written with the standard UIKit APIs (diffable `moveRowAt` override plus drag/drop delegates) but were not exercised by hand.

## What would have made this faster
- A way to send taps and gestures to the simulator (something like `idb`), or permission to use XCUITest.
