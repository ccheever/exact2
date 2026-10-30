# A document app needs chosen files to outlast the launch and to hear when they change

**Status:** Open
**Systems:** storage (`doc:` paths, `storage.fs`), Apple host (security-scoped URLs, Open Recent), web host (File System Access), Linux host, `rules/DEFERRED.md` ("durable capability grants")
**Severity:** P2
**Author:** Claude (Opus 5.5), from Rami Maalouf's Exact Draw report
**Date:** 2026-10-15
**Related:** https://github.com/rami-maalouf/exact-draw/blob/main/docs/exact-experience.md (sections 001 and 006); LLP 1069.010 D1 ("Rejected, for now: a handle that outlives the launch"), D5 (Open Recent), Q2 (bookmarks kept expressible); `docs/reference.md`, "Documents the person chose"; `issues/20261009-macos-orderly-sigterm-quit-hold.md` (stopping a bundled helper)

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): LLP 1069.010.000 accepted 2026-10-15. Stage 1 (watches and expected writes) built 2026-10-16 on macOS, Linux and the web; stage 2 (documents kept across launches) built on macOS 2026-10-18 (the LLP's "As built: stage 2 on macOS"); what remains is below

## Status (2026-10-26)

Stage 2 is built on Linux and the web as well (the LLP's "As built: stage 2
on Linux and the web"), and TypeScript's `atomicWriteFile` carries a kept
file. What remains in this repository is under "What remains (2026-10-26)".
Exact Draw's row is work in Exact Draw's own repository, not this one.

## The consumer

Exact Draw edits Excalidraw drawings on disk: a Contract shell (file explorer,
recents, search) around an embedded Excalidraw editor, on the web and macOS.
It needs three things from the files a person picks:

1. Reopen recent files and folders after a relaunch, from the app's own sidebar.
2. Learn that the open drawing, or a file in the open folder, changed on disk
   (another editor, a sync client, git) and show the change without reloading.
3. List a large folder tree without stalling the interface.

It met none of them in Exact and runs a Bun companion process instead: an
unsandboxed loopback service with absolute paths, a worker for the traversal,
and its own watcher, which both the shell and the editor call. Nothing starts
that service with the app, so the app still needs a terminal to launch.

## What exists (main `11ebf84e9`)

- **Within a session:** a picked file or folder is a `doc:/<n>/<name>` path;
  `readFile`, `stat`, `readdir` and writes beneath a chosen folder work under
  `fs.read doc:/` and `fs.write doc:/` on every host. The handles end with the
  session.
- **Need 3** is reachable already: `readdir` and `stat` walk a chosen folder,
  and a module placed `worker` (LLP 1027.002) walks it off the main thread.
  Not measured on a large tree.
- **Need 1, in part:** on macOS the host records every delivered document with
  `noteNewRecentDocumentURL`, and File ▸ Open Recent reopens one across launches
  through `open-file` (LLP 1069.010 D5). The app cannot list those entries or
  open one from its own interface, and the web and iOS have no equivalent.
- **Need 2:** nothing. No `storage.fs` call reports a change to a path.
- **The companion:** `host.macos.resources` bundles a helper tree into the
  `.app`; nothing launches or supervises it. A native module can, and the
  SIGTERM issue above covers stopping it.

## Ruling

LLP 1069.010.000 was accepted on 2026-10-15 (its Rulings section). On 2026-10-18 Charlie ruled that the in-repo Markdown and LLP readers are stage 1's user, as the LLP's own §2, §5 and §8 name them, so stage 2 starts now on macOS (the bookmark, `<n>` persistence, `recentDocuments()`, `forgetDocument()`, `doc.keep`), then Linux and the web; iOS stays out. D4's hard rows (another program replacing the file at its path, a move) need the Markdown editor and a fixture, not the reader alone.

## What remains (2026-10-18)

Stage 2 is built on macOS: `doc.keep`, the kept set and its `<n>`, the
security-scoped bookmarks, Open Recent as the list, `recentDocuments()`
and `forgetDocument()` from Rust and TypeScript, and the Markdown reader's
Recent list. Left:

- **Linux** (D4, D6): the freedesktop recent-files store as the list, the
  path with its device and inode as the handle, nothing kept outside a
  desktop session. The library's kept set (`exact_data::documents::kept`)
  is the host-independent half.
- **The web** (D4, D6, D7): `FileSystemHandle`s in IndexedDB,
  `requestPermission(id, path, mode)` under a press, the two calls in the
  glue.
- **D4's hard rows by an editor**: the Markdown editor saving a kept file
  with its own atomic replace from TypeScript, and a fixture where another
  program replaces it, on each host.
- **Exact Draw** (§7): reopen its last folder with its companion's file
  service stopped.

## What remains (2026-10-26)

- **D4's hard rows by an editor**: the Markdown editor saving a kept file
  with its own atomic replace (the carry is built and tested beneath it,
  from Rust and TypeScript), and a fixture where another program replaces
  it, driven on each host.
- **A Linux file chooser** (the desktop portal's `FileChooser`): the Linux
  host has none, so a Linux desktop session keeps nothing in practice; the
  keeping path it would call is built (`host/linux/src/kept.rs` `chosen`).
- **Exact Draw** (§7), in its own repository.
