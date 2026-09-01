# Web server exposes stale build files

**Status:** Closed
**Resolution:** Web serving now rejects undeclared, dot, traversal, non-file, and symlink paths and replaces complete output trees.
**Systems:** Web host, Tooling
**Severity:** P1
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1007, LLP 1023.001

`host/web/serve.mjs:15-27` and `host/web/dev.mjs:209-218` serve any regular
filesystem entry lexically beneath the shared `host/web/dist` directory.
`statSync` and `readFileSync` follow symlinks, and builds copy selected outputs
without atomically replacing or cleaning the directory.

The LAN server consequently exposed an existing ignored browser-profile file:
`GET /.chrome-profile/Default/Cookies` returned 200 and 20,480 bytes. The same
profile directory contains history, login-data, and preferences. A symlink
inside `dist` can also escape lexical containment.

Serve from an explicit build manifest or a newly staged immutable directory,
realpath-check every candidate against its real root, and never place browser
profiles under the served tree. Add tests for stale files, dot directories,
symlink files/directories, and a normal asset.
