# Tasks (UIKit) — build notes

## What's there
- **Lists screen**: Today (open tasks due today or overdue), All, then "My Lists", each showing its open-task count.
  Edit mode reorders or deletes lists. Swipe a list to rename or delete it; long-press it to rename, change its colour or delete it.
  The toolbar has "New Task" and "Add List".
- **List screen**: open tasks, then a "N Completed — Show/Hide" row that expands or collapses the completed tasks. The
  expanded/collapsed state is saved per list. The checkbox completes or reopens a task (with a haptic). Swipe left to
  delete, swipe right to complete or reopen. Long-press and drag to reorder (or use Edit for reorder handles); a task
  can only move within its own section (open or completed). The "…" menu renames the list, recolours it or deletes it.
  Deleting asks first and returns you to the main screen.
- **Today** is grouped into Overdue and Today and shows each task's list name. **All** is grouped by list. Both have a
  collapsible Completed section and support swipe and checkbox actions. There's no reordering in Today or All, because
  their order is derived.
- **Task editor** (modal sheet): a title that wraps, with Return ending editing; notes that grow as you type; a Due Date
  switch with an inline calendar; Priority (None/Low/Medium/High); and List, which moves the task to another list. Add/Done is
  disabled while the title is blank. If there are unsaved changes, swiping the sheet down or tapping Cancel asks before
  discarding. When editing, there's also a Delete Task button.
- **Markers**: an overdue task shows a red ⓘ and "Overdue · Sat, Oct 17". A high-priority task shows a red "!!!" before
  its title. Completed tasks are never marked overdue. VoiceOver reads "High priority", "Overdue, due …", and offers
  complete/reopen as a custom action.
- **Persistence**: on first launch the app seeds from the bundled `tasks.json` (`Resources/tasks.json`). After that it keeps a
  JSON store in Application Support and writes it atomically on every change.

## Decisions and guesses
- The data has priorities none/low/high. I added Medium to the picker too, as Reminders has it. Only High gets a strong
  marker; Medium shows a grey "!!" and Low has no marker.
- Today's count includes overdue tasks, as the spec says. Lists keep the order they first appear in the JSON, and colours are assigned from a palette.
- List names must be non-empty and unique (ignoring case). The alert's button stays disabled until they are.
- "Today" is the device's local calendar day, re-checked when the app comes to the foreground or on a
  significant-time-change notification.
- For testing, launch arguments `-screen <Today|All|ListName>`, `-edit <taskID>` and `-new YES` open a screen
  directly. They're harmless, and they're how I took screenshots without being able to tap.

## What was hard / unverified
- With no way to send taps to the simulator, I couldn't exercise gestures (checkbox taps, drag-reorder, swipes, menus) end to end.
  I checked the store logic (add, reorder of a subset, toggle, move between lists, rename, list reorder and delete, Today
  and overdue maths) with a small test harness run in the simulator, and checked every screen visually in light and dark mode
  and at the largest accessibility text size. The drag reorder uses the standard pattern (diffable data source subclass with
  `moveRowAt`, a drag/drop delegate with `.insertAtDestinationIndexPath`, then re-applying the snapshot from the model),
  but no one has touch-tested it.
- Reordering with a diffable data source is fiddly: the table moves the row itself, so the snapshot has to be re-applied
  asynchronously afterwards, and the store-change notification is suppressed during the move.

## What would have made this faster
- A way to tap, swipe and drag in the simulator (idb or an XCUITest runner) to test gestures for real.
