# A document app needs chosen files to outlast the launch and to hear when they change

**Status:** Open; Charlie took the recommendations (watch first, then kept documents) on 2026-10-15; designed in LLP 1069.010.000 (Draft)
**Systems:** storage (`doc:` paths, `storage.fs`), Apple host (security-scoped URLs, Open Recent), web host (File System Access), Linux host, `rules/DEFERRED.md` ("durable capability grants")
**Severity:** P2
**Author:** Claude (Opus 5.5), from Rami Maalouf's Exact Draw report
**Date:** 2026-10-15
**Related:** https://github.com/rami-maalouf/exact-draw/blob/main/docs/exact-experience.md (sections 001 and 006); LLP 1069.010 D1 ("Rejected, for now: a handle that outlives the launch"), D5 (Open Recent), Q2 (bookmarks kept expressible); `docs/reference.md`, "Documents the person chose"; `issues/20261009-macos-orderly-sigterm-quit-hold.md` (stopping a bundled helper)

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

## Ruling owed

1. **Durable handles.** Admit a `doc:` handle that outlives the launch,
   reversing LLP 1069.010 D1's "for now" and taking `doc:` handles out of
   DEFERRED's "durable capability grants". Per host: a security-scoped
   bookmark on Apple (Q2 kept this expressible), a `FileSystemHandle` kept in
   IndexedDB on Chromium (whose permission the browser may ask for again), a path
   on Linux. If yes, it needs an RFC under 1069.010 and a trade.
2. **Change notification.** A `storage.fs` call that reports changes beneath a
   `doc:` path: FSEvents or `NSFilePresenter` on Apple, inotify on Linux, and on
   the web Chromium's `FileSystemObserver` where present, otherwise nothing. It
   is useful within one session, so it does not depend on 1.
3. **Neither.** A document app keeps a companion, and the docs say so with a
   recipe for bundling and starting one.
