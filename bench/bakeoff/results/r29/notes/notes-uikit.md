# Notes (UIKit) — build notes

## What's there
- `AppDelegate.swift`: UIKit app delegate + scene delegate (scene manifest in `EXTRA_PLIST`; the class is
  exported as `@objc(SceneDelegate)` so the plist name doesn't depend on the swiftc module name).
- `NoteStore.swift`: model + JSON persistence. On first launch it copies the bundled `notes.json` (90 notes) into
  Application Support; after that it reads and writes only that file, with an atomic write on every change.
- `NotesListViewController.swift`: inset-grouped table with a diffable data source. The "Pinned" section comes
  first, then "Notes", each sorted by most recently updated. Each row shows the title, a short date (time today,
  "Yesterday", weekday within a week, otherwise a short date) and a one-line preview. Search is a
  UISearchController that matches title or body as you type, ignoring case and accents, with a "No Results" empty
  state. The toolbar shows the count ("90 Notes", or "3 of 90 Notes" while searching) and has a compose button.
  You can swipe right to pin or unpin and swipe left to delete; a long-press menu has both actions too.
- `EditorViewController.swift`: the title wraps and sits in a header inside the body text view, so the whole note
  scrolls as one page at any text size. Edits save on each keystroke. The modification date changes only when the
  text actually changes, which moves the note to the top. Return in the title jumps to the body. The bar has a
  Done button (dismisses the keyboard), pin, and delete (asks for confirmation). The keyboard is avoided using
  `keyboardLayoutGuide`.
- `SelfTest.swift`: developer-only scripted scenarios, started with `-selftest <name>` as a launch argument. It
  does nothing on a normal launch.

## Decisions / guesses
- A new note isn't added to the store until it has some text. Any note that is empty when you leave the editor is
  discarded, the same as in Apple Notes. That covers new notes and existing notes you cleared.
- An untitled note takes its list title from the first line of its body, or shows "New Note" if the body is empty
  too.
- Pinning doesn't change a note's modification date. A pinned note that you edit moves to the top of the Pinned
  section.
- The tint color is yellow, after Apple Notes.

## What was hard
- I had no way to tap the simulator: `simctl` can't send touches, and System Events/AppleScript timed out. So I
  wrote launch-argument scenarios (search, no match, new empty note, typed new note, edit an old note, pin,
  delete) and checked them with screenshots, plus `simctl ui` for dark mode and the AX-XXXL text size.
- My first editor used a single-line UITextField for the title, which truncated at accessibility sizes. I switched
  to a header inside the text view, with the body's `textContainerInset.top` sized to it.

## Not done / not verified
- I never ran the real swipe and long-press gestures, because I couldn't send touches. Their actions call the
  same store methods that the scripted scenarios tested.

## What would have made it faster
- A way to tap or type on the simulator from the command line, such as `idb` or an XCUITest runner.
