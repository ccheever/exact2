# Fieldnotes

A local notebook built with Exact2 and Ibex2 storage: write multiline notes,
pin favorites, search titles and bodies, and save or restore a JSON backup.
There is no account or network service.

On macOS, Fieldnotes opens at 1100 × 760 and remembers its window frame. The
notes list sits beside a full-height editor; each scrolls independently, while
save controls stay visible. Use File → Save note (⌘S) or New note (⌘N). Shortcuts
use the same draft and pending-work guards as the buttons. Backups opens a
separate screen and Back to notes returns to the editor. ⌘F opens search from
either screen. Escape clears/closes search or returns from Backups. New note
clears the search and focuses the title; selecting a saved note focuses its body.
Drafts remain protected: New note stays disabled until saved or discarded.

```sh
node host/web/dev.mjs --app fieldnotes
node host/apple/build.mjs fieldnotes-apple --run
node host/apple/build.mjs fieldnotes-apple --ios --run
```

Save a draft before opening another note, or discard the changes. Backups → Save
backup writes a separate file on this device. Copy the displayed text somewhere
safe to keep an independent copy. Restore accepts pasted backup text or, when
empty, uses the saved file; it validates the complete backup before replacing
notes in a transaction.

`app.contract` owns the UI. `app.ts` uses the generated TypeScript storage types:
SQLite at `app:/data/fieldnotes.db`, filesystem access restricted to
`app:/data/backups`. Native hosts choose the app's directories; the browser uses
IndexedDB and SQLite WASM. Browser data belongs to this origin and browser
profile, so keep the dev server's address/port stable; clearing site data also
clears its notes and local backup. HTTPS or localhost is required. There is no
cross-device sync. The current limits are 1,000 notes, 160 characters per title,
20,000 per body, and 4 MB of backup text.

The native integration test runs the real baked TypeScript app against temporary
SQLite/filesystem storage:

```sh
EXACT_UPDATE_TRUST=development cargo test -p fieldnotes-apple
```

Agent mode deliberately disables persistent storage. Drive the normal browser
page to try persistence; the native test uses isolated temporary directories.
