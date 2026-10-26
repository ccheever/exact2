# @exact/search-index: drives by hand

What the agent cannot drive: the system's search over the app's index on
a phone, and the system asking the app to index again (LLP 1133.023 §11,
§14 stage 2). Record the date, the OS version and what happened beside
each step when you run it.

The app is Shelf (`apps/shelf`): its `index` task sets the books, each
of kind `book`, title the book's title and description its author.

## iPhone

Build and install: `bun host/apple/build.mjs --ios --device --run
shelf-apple`.

1. **A book is found.** Open Shelf once, leave it. Swipe down on the
   Home Screen and type a seed book's title: the book is a result under
   Shelf, with its author. Tap it: Shelf opens on that book.
2. **The index follows the library.** Add *Dune*, leave, search "dune":
   it is there. Remove it in Shelf, search again: it is gone (the mirror
   told Spotlight the difference, not the whole list).
3. **The index outlives a launch.** Restart the phone and, before opening
   Shelf, search a seed book: still found (the entries never expire;
   `expirationDate` is the distant future).
4. **Indexing again.** Settings > Siri > Shelf, turn Show App in Search
   off and on, then search a seed book without opening Shelf. Record
   whether it is found, and whether the log (subsystem `dev.exact`,
   category `search`) says `reindex all: N entries from the mirror`.
   Apple documents no way to make the system ask, so this step may show
   nothing.
5. **Locked.** Lock the phone right after a restart and search: no Shelf
   book shows until the first unlock (the index's protection class is
   complete until first authentication).

## Mac

Build: `bun host/apple/build.mjs --run shelf-apple`.

6. **Spotlight.** Cmd-Space, type a seed book's title: the book is a
   result under Shelf; Return opens it.

## Android phone

7. **The mirror only.** Android shows a third-party app's index nowhere,
   so there is nothing to see in the launcher's search. In Shelf, add a
   book, force-stop Shelf, open it: `contentIndexEntries` still holds the
   book (the module's file in the app's files).
