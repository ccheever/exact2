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
bun host/web/dev.mjs --app fieldnotes
bun host/apple/build.mjs fieldnotes-apple --run
bun host/apple/build.mjs fieldnotes-apple --ios --run
```

Save a draft before opening another note, or discard the changes. Backups → Save
backup writes a separate file on this device. Copy the displayed text somewhere
safe to keep an independent copy. Restore accepts pasted backup text or, when
empty, uses the saved file; it validates the complete backup before replacing
notes in a transaction.

`app.contract` owns the UI. `app.ts` owns notes, search and restore; `data/`
implements `backupNotes` in Rust behind the same data-source interface. Both use
the same storage and grants. Mutation revisions use the runner's app-owned
`fieldnotes.revision` Store entry, so repeated saves/deletions still invalidate
the library after a language change or module reload:
SQLite at `app:/data/fieldnotes.db`, filesystem access restricted to
`app:/data/backups`. Native hosts choose the app's directories; the browser uses
IndexedDB and SQLite WASM. Browser data belongs to this origin and browser
profile, so keep the dev server's address/port stable; clearing site data also
clears its notes and local backup. HTTPS or localhost is required. There is no
cross-device sync. The current limits are 1,000 notes, 160 characters per title,
20,000 per body, and 4 MB of backup text.

The library returns titles, pins and short previews; selecting a note loads its
full body separately. An unfiltered list fetches short text prefixes. If whitespace
or an embedded NUL leaves a prefix too short for its preview, it fetches the needed
full rows in batches of 32. Exact search still scans all bodies in batches of 32
and uses JavaScript's Unicode lowercase substring matching. These replies keep a
full notebook below the storage result limit. Batches are independent reads,
not one cross-window snapshot; fallback rows use their current title, body and pin,
and rows deleted before fallback are omitted. Backups keep their single-snapshot
behavior and separate size limit. When every preview needs fallback, the extra
prefix query adds overhead; the gain is for lists whose previews fit the prefix.
While a selected note loads, the editor is read-only; drafts and edits made
while saving stay protected. The loaded editing snapshot and unsaved draft survive
a code reload; a reload that interrupts opening a note offers Retry. Preview truncation never splits a surrogate pair.

Both language implementations check the current database's schema before creating
it. Reads of an initialized notebook no longer issue a no-op write, which avoids the
browser's whole-database export on library, selected-note and backup reads. The
check is repeated for each opened file so deleting or replacing the database does
not leave a cached initialization flag behind. Actual writes still export the
whole database on the browser; this does not change that persistence backend.

`app.json` places the TypeScript module on a worker (LLP 1027.002:
`"typescript": { "placement": "worker" }`); the Rust half stays on `main`. Both
keep `fieldnotes.revision`, so the composer orders every call of either half
into one queue, each turn running against the store as committed and its writes
landing through the runner. On native the module has an owner thread of its
own; on the web it runs in a dedicated Worker instead of the private iframe.
`EXACT_TYPESCRIPT_PLACEMENT=main` (or `EXACT_RUST_PLACEMENT=worker`) at bake
overrides the manifest for a measurement; the compatibility id carries the
result. The Chrome drive in `web/tests/worker.rs` reports the page's frame gaps
while a backup of 1,000 notes runs beside wheel scrolling, at 1× and 4× CPU
throttling, against the display's own frame interval.

The native integration test runs the real baked TypeScript app and the Rust
backup source against temporary SQLite/filesystem storage. It also drives the
notebook on every placement pair and the host's own dispatch and pump loop. It moves backup
between languages through the Rust module ABI, compares the complete answer and
file, restarts, restores the Rust backup in TypeScript, and checks refusal and
size errors. It also drives backup followed by delete, reload and another delete
through the runner to verify that the shared counter refreshes the visible library:

```sh
EXACT_UPDATE_TRUST=development cargo test -p fieldnotes-apple
```

Agent mode deliberately disables persistent storage. Drive the normal browser
page to try persistence; the native test uses isolated temporary directories.
