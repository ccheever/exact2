# Code review: the variant gallery on the JS dev loop, G3, round 1 of 3 (grok)

- **Family:** xAI. Grok via its CLI, reasoning effort xhigh, Edit and Write denied, a throwaway worktree.
- **Method:** one brief (sha256 `8786a563602951a4bc89bcdbfb561e4dc2afdbc3d3c7385c98beafec8e3622b4`), shared by both reviewers, each blind to the other's review; HEAD `d052536b35`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited (from its findings; its narration before them left out).
- **Verdict:** 2 BLOCKER, 6 SHOULD-FIX, 4 NIT.
- **Disposition:**
  All taken, in the round 1 commit:
  - Astra 1 (BLOCKER), Grok 5 (maps): the gallery keeps its own maps (its last 4, by digest), which the loop's source-map lookup asks after the app's. The app's store is untouched.
  - Astra 2 (BLOCKER, the inspect table): inspect-dev.js asks for `dev-inspect.json` beside the page (its `<base>`). The gallery answers `/__dev/gallery/dev-inspect.json` from its own plan, through the loop's `inspectTable(digest, dist)`.
  - Astra 3 (BLOCKER), Grok 7 (a kept scrub, or a design kept during one): the gallery hears each committed design (`designKept(digest)`, wherever the loop sets its committed snapshot and at a scrub's restore), not the batch sites. It builds again when the digest moved.
  - Astra 4 (BLOCKER, a group forgotten), Grok 3: a step ends only when its whole group is gone (polled, SIGKILL after 5 s, as the app's build). stop() falls back to the child's own kill, with a 5 s deadline.
  - Astra 5, Grok 1 (BLOCKER, assets): `/__dev/gallery/assets/…` is served from the app's dist (the gallery's assets are the app's). Tested.
  - Astra 6, Grok 2 (BLOCKER, a failed first build): a page asking again remakes a failed gallery, and `active` is tracked apart from builds. Tested: a gallery that can't be made, then can.
  - Astra 7, Grok 4 (idle cleanup): the quiet-period stop is armed on every request with no page of it open, and by every stream's close.
  - Astra 8 (seq reset): seq only grows; an idle stop leaves its build to be made again.
  - Astra 9 (a start lost in a stop): a start asked while a stopped run winds down runs once it does.
  - Astra 10, Grok 8 (errors escaping): refresh catches and says them on the gallery's stream.
  - Astra 11, Grok 11 (the cleanup test): a new test interrupts the loop alone (SIGINT) mid-build and checks the gallery's recorded processes are gone. A gallery that can't be made says so on its stream only.
  - Grok 6 (cargo lock): the built contract compiler is run directly (Cargo only when it's missing or doesn't know `gallery`), and the build gets the loop's fresh JS compiler (EXACT_WEB_COMPILER), as the app's does.
  - Grok 9: the wasm loop's 404 also answers `/__dev/gallery` with no trailing slash.
  - Grok 10: the comment names host/web-js/dev-gallery.mjs.
  - Grok 12: a page with no `<base href="/">` is given one after `<head>`.

1. **BLOCKER** `host/web-js/dev-gallery.mjs:78` — The gallery keeps `assets` as a symlink into `live`, and `host/web-js/build.mjs` copies that symlink into the dist (`cpSync` of a directory symlink copies the link, not the files). `buildTreeFile` serves a file only when `realpathSync(file) === file` (`host/web/serve.mjs:705`), so every `/__dev/gallery/assets/…` request is a 404. Font faces and preloads are relative `assets/…` URLs; with `<base href="/__dev/gallery/">` they resolve under that prefix and do not load. The test only fetches `./app.js`.

2. **BLOCKER** `host/web-js/dev-gallery.mjs:100` — A failed first build leaves `seq === 0` and `failed` set. `ensure()` will not start another build while `failed` is set, and `appChanged()` (`dev-gallery.mjs:159`) only refreshes when `seq > 0 || busy`. The error page holds `/__dev/gallery/stream` open, so the idle timer that clears `failed` (`dev-gallery.mjs:126`) never runs. Reloading the page or publishing an app edit does not rebuild it. Recovery is to close the page and wait out the idle period with no further gallery request.

3. **SHOULD-FIX** `host/web-js/dev-gallery.mjs:110` — The group kill has no fallback to the child, unlike the loop's `stopGroup`. On a throw, `child` is already null, so the `exit` handler will not kill it either, and `stop()` waits on `'exit'` with no deadline. A failed kill hangs the loop's SIGINT/SIGTERM path (`dev.mjs:1040` awaits `gallery.stop()`) and can leave the cargo or build process running.

4. **SHOULD-FIX** `host/web-js/dev-gallery.mjs:99` — `ensure()` clears the idle timer on every gallery request, including a static file, and only a stream's `close` arms it again. One request after the last stream closes cancels the quiet-period stop for good, so an in-flight `cargo run` or `build.mjs` is not killed when the page is gone.

5. **SHOULD-FIX** `host/web-js/dev.mjs:205` — Gallery plans are stored in the app's `pageMaps`, one map capped at 8. Each gallery build can evict an app plan map (and the reverse), so an outline for a plan that is still running can miss its map. Digests do not overwrite each other. The retention window is shared.

6. **SHOULD-FIX** `host/web-js/dev-gallery.mjs:69` — Every refresh runs `cargo run -p contract` in the repo, which takes the workspace target lock the design reader's `cargo build --profile host-dev` uses. The JS build is spawned without `EXACT_WEB_COMPILER` (`dev-gallery.mjs:83`), so a stale debug compiler becomes a second `cargo run` on that same lock. `appChanged()` is not awaited, so the edit that just published is not delayed. The next reader rebuild can be.

7. **SHOULD-FIX** `host/web-js/dev.mjs:496` — A value-only design edit that lands while a scrub is previewing updates `design` and sends no batch (`!scrubbed`), so the gallery hook does not run. The verifying kept build then diffs against that already-updated design, sees no tokens, and does not hook either (`dev.mjs:573`). Scrub preview, restore, and patch correctly do not hook. After that scrub ends, the app shows the new design and the gallery stays on the old build until some later publish.

8. **SHOULD-FIX** `host/web-js/dev-gallery.mjs:67` — `refresh()` only turns a non-zero exit into a gallery-stream `{error}`. A throw (`readFileSync`, `symlinkSync`, `replaceBuild`'s lock) rejects the promise `appChanged()` does not catch, so the failure is an unhandled rejection on the app's loop rather than a frame on the gallery stream.

9. **NIT** `host/web/dev.mjs:1340` — The explanatory 404 matches `/__dev/gallery/…` only. `/__dev/gallery` with no trailing slash falls through to the wasm loop's ordinary 404. The JS loop treats both as the gallery. The gate still runs first.

10. **NIT** `scripts/tamber-builds.mjs:11` — The comment names `host/web/dev-gallery.mjs`. The module is `host/web-js/dev-gallery.mjs`. The 1.15 protocol line and the Registration section name `page` correctly, and the field is stored and listed only, with no effect on pin, inspect, hold, or navigate.

11. **NIT** `host/web-js/dev-gallery.test.mjs:121` — "Nothing left" is `pgrep -f` of `<cache>/live`. `cargo run … gallery … -o <cache>/source` does not contain that path, so an orphaned contract process passes the check. The test also never requests an asset, never fails a build, and never opens the wasm loop.

12. **NIT** `host/web-js/dev-gallery.mjs:153` — The base rewrite is an exact `<base href="/">` replace, which matches what `build.mjs` writes today. A miss leaves `base href="/"` in place, and the gallery page then loads `/app.js` from the app.
