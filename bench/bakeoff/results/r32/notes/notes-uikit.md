# Notes (UIKit): build notes

## What's here
- `Sources/AppDelegate.swift`: the UIKit app delegate and scene delegate. The scene manifest is in `EXTRA_PLIST` in `app.env`, and the delegate class is exported as `@objc(SceneDelegate)`, so the plist doesn't depend on the module name.
- `Sources/NoteStore.swift`: the `Note` model and the store. On first launch it seeds from the bundled `Resources/notes.json` (a copy of `data/notes.json`). After that it saves JSON to Application Support: writes are batched for 0.5 s, then flushed whenever the app resigns active, goes to the background or quits.
- `Sources/NotesListViewController.swift`: an inset-grouped table driven by a diffable data source.
  - Sections: "Pinned" first, then "Notes". Headers appear only when there are pinned notes.
  - Each row shows the title, a short date (time today, "Yesterday", the weekday within the last week, otherwise a short numeric date) and a one-line preview.
  - Search is live, matches title or body, and ignores case and accents.
  - Swipe right to pin or unpin; swipe left to delete. A long press opens a menu with the same actions.
  - The toolbar shows the note count ("N Notes", or "N Results" while searching) and the compose button.
  - Empty states for no search results and for no notes.
- `Sources/NoteEditorViewController.swift`: a title text view and a body text view, with no Save button.
  - Every keystroke updates the note and its `updated` date, so the note moves to the top of the list.
  - Return in the title moves to the body. Pasted line breaks in the title become spaces.
  - Done appears while the keyboard is up. A "…" menu has Pin/Unpin and Delete.
  - A new note only joins the store once it has text. Any note that is empty when you leave the editor is discarded.

## Tested on the simulator
I used a temporary in-app harness, driven by a launch argument and removed before the final build, plus screenshots and the saved JSON file.
- Search, including no results.
- Editing moves the note to the top.
- A new note with text is kept.
- An empty new note is discarded (count stays the same).
- Swipe pin and swipe delete.
- Changes survive a relaunch.
- Dark mode and the largest accessibility text size.

The app was uninstalled and reinstalled at the end, so it starts fresh from the 90 seed notes.

## Hard parts and guesses
- `simctl` can't tap or type, so testing interactions needed the in-app harness above.
- The diffable data source owns the table's data source. Section titles and `canEditRow` have to be overridden in a subclass of it, not in the view controller.
- iOS 26 toolbar: the count label needs `hidesSharedBackground` to avoid a glass bubble. Toolbars don't grow with Dynamic Type, so the label uses the Large Content Viewer instead.
- Guess: an *existing* note that is fully emptied is also deleted on leave, as Apple Notes does. The spec only names new notes. Notes left empty because the app was killed are removed on the next launch.
- Guess: a note's date changes only when its text changes. Pinning doesn't count as an edit and doesn't reorder notes within a section.
- Guess: no confirmation before deleting, which is standard for swipe-to-delete.

## Not done
- No undo for delete.
- No rich text.
- No iPad or landscape layout (the skeleton is portrait iPhone only).

## What would have made this faster
A way to send taps and text to the simulator (an idb- or XCUITest-style driver), so the real interactions could be tested without an in-app harness.
