# Notes (UIKit) — build notes

**What's in it**
- The list is a `UICollectionView` inset-grouped list with a diffable data source. It has a "Pinned" section when any notes are pinned. Notes are sorted newest first, with ties broken by id. Each row shows the title, plus a date and a one-line preview. The date is the time for today, "Yesterday", the weekday within the last week, and a short date otherwise.
- Search uses `UISearchController` and filters as you type, matching title or body with `localizedStandardContains` (ignores case and accents). There's an empty state when nothing matches.
- The toolbar shows the count: "N Notes", or "N Found" while searching. It also holds the compose button.
- Swipe right on a row to pin or unpin it, swipe left to delete, or long-press for a menu with both. VoiceOver custom actions do the same.
- The editor has a separate title and body, which are non-scrolling text views inside a scroll view pinned to `keyboardLayoutGuide`. Return in the title moves to the body, and the editor keeps the cursor visible as you type. A Done button appears while editing, and the ⋯ menu has Pin/Unpin and Delete.
- Edits save as you type and update `updated`, which moves the note to the top of its section. Opening a note without changing it doesn't move it.
- A new note is only created once it has non-blank text. A note left blank (new or existing) is deleted when you leave the editor. Blank notes are also dropped on load, in case the app was killed mid-edit.
- Notes are saved as JSON in Application Support. Saves are batched 0.4 s after a change, and pending saves are written when the app goes to the background or leaves a note. On first launch the app copies in `Resources/notes.json` (from `data/`).
- Dark mode and Dynamic Type: system colours and text styles throughout. At accessibility text sizes, list rows wrap to 3 lines.

**Testing.** There's no UI automation, so I added launch arguments: `-newNote`, `-openNote <id>`, `-type <text>`, `-popAfterType` and `-search <q>`. With those plus `simctl` screenshots I checked: creating a note, edit-then-relaunch persistence, the moved-to-top order, discarding a blank note, search, dark mode and the AX3 text size. I didn't run the swipe actions or context menu by hand, because simctl can't send touches. They're standard UIKit APIs.

**Guesses.**
- The spec only says to discard an empty *new* note. I also delete an existing note that's left blank, as Apple Notes does.
- Pinning doesn't change a note's `updated` date.
- On iOS 26, toolbar items that are labels need `hidesSharedBackground` so they don't get a glass bubble.

**What would have helped.** A simulator tap/swipe tool (like idb) to test gestures directly.
