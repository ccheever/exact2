# @exact/quick-actions and Shelf's intents: drives by hand

What the agent cannot drive: a launcher's or a taskbar's own menu, Siri,
and a phone (LLP 1133.023 §11 and §14). Each step says what to see.
Record the date, the OS version and what happened beside each step when
you run it.

The Apple host's lines are in the system's log under subsystem
`dev.exact`, category `tools` (`log stream --predicate 'subsystem ==
"dev.exact"'` on the Mac, Console.app for a phone): each call's arrival,
whether a session was booted for it, and its outcome.

The app is Shelf (`apps/shelf`): the manifest's Add Book (`/add`), its
`index` task's Continue *title*, and the tools Log Pages and Add to Shelf.

## iPhone

Build and install: `bun host/apple/build.mjs --ios --device --run
shelf-apple`.

1. **The Home Screen's menu, cold.** Swipe Shelf away in the app
   switcher. Long press its icon: Add Book is in the menu with a plus,
   and Continue *Piranesi* (the book last opened) below it once Shelf has
   run once. Tap Add Book: Shelf opens on the add sheet.
2. **Warm.** With Shelf in the background on its library, long press and
   tap Add Book: the add sheet comes up over the library.
3. **Spotlight.** Swipe down on the Home Screen and type a seed book's
   title (*Piranesi*): the book is a result under Shelf. Tap it: Shelf
   opens on that book. Add *Dune* in Shelf, leave, search "dune": it is
   there; remove it in Shelf, search again: it is gone.
4. **Shortcuts.** In the Shortcuts app, add an action, search "Log
   Pages": Shelf's Log Pages is listed. Make a shortcut of it, pick a
   book (the picker lists Shelf's books, searched by title or author) and
   a page, and run it with Shelf swiped away: it reports done, and opening
   Shelf shows that page on the book. Run it again with Shelf open on the
   book: the page changes in place.
5. **Siri.** Say "Log pages in Shelf": Siri asks for the book and the
   page, then reports done; Shelf shows the page.
6. **Locked.** Lock the phone and run the shortcut from Siri: it asks to
   unlock first (the face's default, `requiresAuthentication`).

## Mac

Build: `bun host/apple/build.mjs --run shelf-apple`.

7. **The Dock menu.** Right-click Shelf's Dock icon: Add Book and Continue
   *title* are above the system's items. Choose Add Book: Shelf comes to
   the front on the add sheet.
8. **Spotlight.** Cmd-Space, type a seed book's title: the book is a
   result under Shelf; Return opens it.
9. **Shortcuts, Shelf quit.** Quit Shelf. In Shortcuts make a one-action
   shortcut of Log Pages and run it: record what opens (a window, nothing,
   the app in the Dock) beside what a hand-built Mac app's intent opens
   (§15 question 5), and that Shelf shows the page when opened.

## Android phone

Build: `bun scripts/agent-android.mjs build shelf --apk`, then `adb
install -r` the APK it names.

10. **The launcher's menu.** Long press Shelf's icon: Add Book and, after
    Shelf has run once, Continue *title*. Tap Add Book, cold and with
    Shelf in the background: Shelf shows the add sheet each time.
11. **A pinned shortcut.** Drag Continue *title* from the menu to the
    Home Screen; tap it: Shelf opens on the book.
12. **A URL from outside.** `adb shell am start -a
    android.intent.action.VIEW -d /add -n <package>/dev.exact.app.MainActivity`,
    cold and warm: Shelf lands on the add sheet. `-d /nowhere`: Shelf
    opens on its start, and logcat says `open /nowhere: no route matches
    it` when it was running.

AppFunctions (Log Pages from Gemini) are not built (LLP 1133.023 stage
3's as-built note).

## Windows (the NucBox, signed-in console session)

Build: `bun host/windows/build.mjs shelf-windows`.

13. **The jump list.** Run Shelf once, then right-click its taskbar
    button: Tasks lists Add Book. Close Shelf, choose Add Book from the
    jump list: Shelf opens on the add sheet.
14. **A second launch.** With Shelf open, choose Add Book again: today a
    second window opens; the hand-off to the running one is LLP
    1133.025's.

## Linux desktop

Build: `bun scripts/build-linux.mjs shelf`; it writes
`com.exact.shelf.desktop` beside the binary.

15. **The desktop entry.** Copy it to `~/.local/share/applications/`.
    Right-click Shelf in the dock or the app grid (GNOME, KDE): Add Book
    is an action; choose it: Shelf opens on the add sheet.

## Chrome

16. **The installed app's menu.** Install Shelf from Chrome (the address
    bar's install button) on a desktop or Android: right-click (or long
    press) its icon: Add Book opens the add sheet in the app's window.
17. **WebMCP.** In a Chrome with `document.modelContext` (its testing
    flag), open Shelf: an agent in the browser lists Log Pages and Add to
    Shelf, and Log Pages run from it shows the page.
