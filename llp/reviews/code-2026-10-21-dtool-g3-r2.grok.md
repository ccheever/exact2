# Code review: the variant gallery on the JS dev loop, G3, round 2 of 3 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `34ea4f0c7b5535fa25e34e05b2952ef78d2d0dd1c61d7fd57ce4b41ac71c4548`), shared by both reviewers, each blind to the other's review; HEAD `232dc81055`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** nothing blocks (3 SHOULD-FIX, 2 NIT); round 1's all closed.
- **Disposition:**
  All taken, in the round 2 commit:
  - Astra 1 (BLOCKER), reload1's 09:40 should-fix (the gallery's work before the app's publication): each `designKept` is deferred with setImmediate past the batch, the restore's included. The reload's `appChanged` already follows its push. A source test pins all three.
  - Astra 2 (BLOCKER, inspect tables outside its lifecycle), Grok 2 (shared table cache): the gallery makes its own plan's inspect table, its last two, with the loop's own command (`tableTool`), as a process it owns. stop() kills every process it owns (`procs`), its tables' included.
  - Astra 3, Grok 4 (a needless rebuild at a scrub's restore): each build records the committed design it starts on, and a `designKept` of that digest is no news.
  - Astra 4, Grok 1 (the first page built twice): a page joining the build under way adds none. Only a new activation, or a failure, asks again.
  - Astra 5, Grok 3 (Cargo contention, a stale binary): the contract compiler runs directly only while fresh by its dep-info, as scripts/verify.mjs does; otherwise Cargo builds it in the gallery's own target (`<cache>/cargo`), never the app's or its lock.
  - Astra 6, Grok 5 (cleanup proof): a new test runs the gallery with stand-in commands. It stalls the generator and the builder in turn, each holding a child of its own, and checks every recorded pid is gone, at stop and after the quiet period.
  - Astra 7: a page with no `<head>` gets its `<base>` after the doctype.
  - reload1's nit: `gallery` is declared before the closures that call it.

1. **Closed** — Gallery maps are their own four-entry store (`host/web-js/dev-gallery.mjs:119`). The app store is unchanged (`host/web-js/dev.mjs:204`). Lookup checks the app, then the gallery (`host/web-js/dev.mjs:879`).
2. **Closed** — The inspector fetches `dev-inspect.json` beside the page (`host/web-js/inspect-dev.js:27`). The gallery serves its own plan (`host/web-js/dev-gallery.mjs:179`).
3. **Closed** — A committed design calls `designKept` on the fast path (`host/web-js/dev.mjs:494`), a kept build (`host/web-js/dev.mjs:570`), and scrub restore (`host/web-js/dev.mjs:842`). Preview, patch, and restore-without-a-scrub do not. A full publish still calls `appChanged` after the reload (`host/web-js/dev.mjs:607`).
4. **Closed** — `step` keeps the child until `kill(-pid, 0)` returns `ESRCH`, and sends `SIGKILL` after 5 seconds (`host/web-js/dev-gallery.mjs:69`).
5. **Closed** — `/__dev/gallery/assets/…` is read from the app dist (`host/web-js/dev-gallery.mjs:190`).
6. **Closed** — `active` is not `seq`. A failed or stopped gallery is built again on the next request (`host/web-js/dev-gallery.mjs:136`).
7. **Closed** — Every request with no open stream arms the idle timer (`host/web-js/dev-gallery.mjs:129`), and so does each stream close (`host/web-js/dev-gallery.mjs:162`).
8. **Closed** — `seq` only increases (`host/web-js/dev-gallery.mjs:106`). Idle does not reset it.
9. **Closed** — A request during a build sets `again` (`host/web-js/dev-gallery.mjs:139`). The run’s `finally` starts that build even if this run was stopped (`host/web-js/dev-gallery.mjs:114`).
10. **Closed** — `refresh` catches and reports on the gallery stream (`host/web-js/dev-gallery.mjs:109`).
11. **Closed** — The test fails a real generator run, then sends `SIGINT` only to the loop and checks those pids before the backstop (`host/web-js/dev-gallery.test.mjs:138`).

Review B: the group kill falls back to the child and gives up after 5 seconds (`host/web-js/dev-gallery.mjs:148`). The direct `contract` binary and `EXACT_WEB_COMPILER` are wired (`host/web-js/dev-gallery.mjs:85`, `host/web-js/dev.mjs:723`). `/__dev/gallery` without a slash is the wasm loop’s 404 (`host/web/dev.mjs:1340`). The comment path is `host/web-js/dev-gallery.mjs` (`scripts/tamber-builds.mjs:11`). A page with no `<base href="/">` gets one (`host/web-js/dev-gallery.mjs:207`).

The default `client(n, revision)` expansion matches the pre-gallery script byte for byte. `page` is stored and listed only (`scripts/tamber-builds.mjs:130`, `scripts/tamber-builds.mjs:187`). The 1.15 line names it (`docs/tamber-protocol.md:126`, `docs/tamber-protocol.md:1298`). The gate runs before any gallery work (`host/web-js/dev.mjs:887`). Hooks are not on scrub preview. Cache paths sit under `node_modules` or `target`, both of which the watchers skip.

## Round 2

1. **SHOULD-FIX** — `host/web-js/dev-gallery.mjs:139` — The first page always builds twice. The HTML request starts the build (`built` is still false). The waiting page’s stream then calls `ensure`, sees `busy && !built`, and sets `again`. The first build’s `finally` starts a second one. The waiting page reloads, then the second build reloads it again. Queue `again` for a new activation or a failure, not because the build already running has not published yet.

2. **SHOULD-FIX** — `host/web-js/dev.mjs:389` — Gallery inspect tables share the app’s four-slot `tables` map (`host/web-js/dev.mjs:371`). Each gallery plan digest is inserted by `inspectTable(digest, dist)` (`host/web-js/dev-gallery.mjs:182`). After four gallery inspections the app’s table is dropped. The next app inspect rebuilds it (the comment says that takes seconds on a large app). Digests do not overwrite each other. Give the gallery its own table cache.

3. **SHOULD-FIX** — `host/web-js/dev-gallery.mjs:85` — A present `target/debug/contract` is run as-is. Cargo runs only when the file is missing or exits 2. The design reader builds `exact-web-js` in the `host-dev` profile (`host/web-js/dev.mjs:322`); it does not refresh `debug/contract`. A stale binary that still accepts `gallery` (exit 1, or a successful old generator) never falls back, so the gallery stays wrong until that file is deleted. Use it only when its depfile is fresh, as `scripts/verify.mjs` does.

4. **NIT** — `host/web-js/dev-gallery.mjs:217` — `design` starts null and a successful build does not record the digest it published. The first scrub restore calls `designKept` (`host/web-js/dev.mjs:842`) with the digest already on screen, and that compares unequal, so the gallery rebuilds once with no edit.

5. **NIT** — `host/web-js/dev-gallery.test.mjs:129` — Cleanup is still “pids whose command line contains the cache path.” `git check-ignore` does not, and the sample catches whichever of the generator or `build.mjs` is alive, not both. Idle is only a log line, after the build has finished.
