# Tasks (UIKit): notes

## What's built
- **Main screen:** Today and All with their open-task counts, then "My Lists", each with a colour, an icon and its open-task count. Edit mode reorders lists. You can rename or delete a list by swiping, from its long-press menu, or by tapping its info button in edit mode. The + button adds a list. Deleting a list asks for confirmation and says how many tasks go with it. Empty or duplicate names (case-insensitive) can't be saved.
- **List screen:** open tasks, then a collapsible "Completed (n)" section; whether it is collapsed is saved for each list. The checkbox completes or reopens a task, and swiping right does the same. Swiping left deletes. You can reorder by long-press-dragging, or with Edit Tasks in the ••• menu, which also has Rename List and Delete List. Rows can't be dragged between the open and completed sections.
- **Today and All:** read-only smart lists that can't be reordered, with each row showing its list name. Today covers open tasks due today or earlier, plus the completed ones in that range, so a task you tick off moves to Completed instead of vanishing. A new task added from Today defaults to being due today.
- **Editor:** a sheet with title, notes, due date (a switch plus an inline calendar), priority (None/Low/High) and list. You can move a task to another list from here. Add/Done stays disabled until the title has text. If there are unsaved changes, dismissing it asks whether to discard them. When editing, there is a Delete Task button.
- **Markers:** an overdue date is shown in red with a ⚠︎ icon and the word "Overdue" (completed tasks are never marked overdue). High priority shows a red "!!!" and low priority a grey "!".
- **Saving:** the data is stored as JSON in Application Support and written after every change. On first launch it is seeded from the bundled `tasks.json`.
- **Display and accessibility:** works in dark mode and at the accessibility text sizes. At those sizes the title wraps (it is a text view, not a field), and the Priority and List rows stack vertically. VoiceOver reads each row as one element and offers Complete/Reopen as a custom action.

## Hard or guessed
- Due dates are calendar days ("yyyy-MM-dd"). "Overdue" means before today in the device's time zone. I compare them as strings to avoid time-zone drift.
- The spec doesn't say what Today shows for completed tasks, so the rule above is my choice. Low priority is shown (grey "!") even though the spec only asks for high priority to be marked.
- Setting `contentConfiguration` on a cell after adding custom subviews replaced those subviews. That is why the Priority and List buttons first rendered blank.
- For reordering with a diffable data source, `moveRowAt` calls `super` and then writes the new order back to the model. A reordered task reuses the existing order slots, so moves within the open tasks don't disturb where the completed ones sit.

## Testing
No tool to tap the simulator was available. To test, the app takes launch arguments: `-screen today|all|<list name>`, `-bottom 1` (scrolls to the end), `-edit <id>`, `-new 1`, and `-selftest 1`. The self-test drives the real code paths: complete, reorder via the data source, delete, collapse, and create/rename/delete a list. I checked the results with screenshots and the console log, and confirmed they survived a relaunch. Afterwards I reinstalled the app so it starts from the shipped data. Dragging and swiping by hand were not exercised, only the code paths behind them.

## What would have made this faster
A way to send taps and swipes to the simulator, such as idb or an XCUITest runner.
