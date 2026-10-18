# A document app needs chosen files to outlast the launch and to hear when they change

**Status:** Open
**Systems:** storage (`doc:` paths, `storage.fs`), Apple host (security-scoped URLs, Open Recent), web host (File System Access), Linux host, `rules/DEFERRED.md` ("durable capability grants")
**Severity:** P2
**Author:** Claude (Opus 5.5), from Rami Maalouf's Exact Draw report
**Date:** 2026-10-15
**Related:** https://github.com/rami-maalouf/exact-draw/blob/main/docs/exact-experience.md (sections 001 and 006); LLP 1069.010 D1 ("Rejected, for now: a handle that outlives the launch"), D5 (Open Recent), Q2 (bookmarks kept expressible); `docs/reference.md`, "Documents the person chose"; `issues/20261009-macos-orderly-sigterm-quit-hold.md` (stopping a bundled helper)

Status note (moved verbatim off the **Status:** line by `bun scripts/issue.mjs`; cdcstack issue statuses are exactly `Open` or `Closed`): LLP 1069.010.000 accepted 2026-10-15. Stage 1 (watches and expected writes) built 2026-10-16 on macOS, Linux and the web; stage 2 (documents kept across launches) waits for stage 1 to have a user

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
