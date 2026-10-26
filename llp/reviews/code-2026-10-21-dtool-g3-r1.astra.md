# Code review: the variant gallery on the JS dev loop, G3, round 1 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `8786a563602951a4bc89bcdbfb561e4dc2afdbc3d3c7385c98beafec8e3622b4`), shared by both reviewers, each blind to the other's review; HEAD `d052536b35`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 4 BLOCKER, 7 SHOULD-FIX.
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

This round has blockers.

1. **BLOCKER — Gallery builds evict the running app’s source map.** [host/web-js/dev.mjs:205](/tmp/rv-dt-g3r1/host/web-js/dev.mjs:205)  
   Both apps use the same eight-entry `pageMaps`. After eight distinct gallery builds during value-only edits, the unchanged app’s map is evicted: kept app builds never refresh it. App inspection becomes read-only and outlines lose their source mapping. Give gallery maps independent retention.

2. **BLOCKER — Gallery inspection requests the app’s inspect table.** [host/web-js/dev.mjs:722](/tmp/rv-dt-g3r1/host/web-js/dev.mjs:722)  
   The injected inspector still fetches `/dev-inspect.json`, whose handler builds the table from the app’s `dist`. Its plan-digest check rejects the gallery’s plan, so gallery selection fails and holds cannot apply. The gallery needs its own inspect-table route and client configuration.

3. **BLOCKER — Keeping a scrub can leave the gallery unchanged.** [host/web-js/dev.mjs:500](/tmp/rv-dt-g3r1/host/web-js/dev.mjs:500)  
   When release commits the final preview, `from = scrubbed` already equals the committed tokens. `tokens.length` is zero, skipping both this hook and the kept-build hook at line 579. Since previews deliberately never reach the gallery, it retains the old design. Notify it after committed publication even when the app needs no additional batch.

4. **BLOCKER — A process group is forgotten when its leader exits.** [host/web-js/dev-gallery.mjs:58](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:58)  
   `step()` clears `child` on `exit`, although descendants may remain. The builder starts asynchronous Git commands and can exit on compilation failure before awaiting them. Any surviving Git process then escapes both `stop()` and the exit handler. Retain ownership until the group is gone; the app’s existing build path explicitly handles this case.

5. **SHOULD-FIX — Gallery assets become unservable symlinks.** [host/web-js/dev-gallery.mjs:78](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:78)  
   `live/assets` is a symlink. The builder copies it without dereferencing, which preserves the link by default; `buildTreeFile` then rejects files reached through it. Relative image/font requests under the gallery prefix return 404. Materialize regular asset files in the gallery build while preserving the serving restriction. ([`cpSync` options](https://nodejs.org/api/fs.html#fscpsyncsrc-dest-options))

6. **SHOULD-FIX — A failed first build cannot recover on an edit.** [host/web-js/dev-gallery.mjs:159](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:159)  
   After initial failure, `seq === 0`, `busy === false`, and `failed` is set. `appChanged()` refuses to refresh, while `ensure()` also refuses because of `failed`. Fixing the source or reloading the page does nothing until all streams disappear long enough for idle reset. Track activation independently of successful builds.

7. **SHOULD-FIX — Requests without a stream can disable idle cleanup indefinitely.** [host/web-js/dev-gallery.mjs:99](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:99)  
   Every static request clears the idle timer, but only a stream’s closure arms it. Fetching the page without opening SSE—or fetching an asset after the last stream closes—leaves no timer. Subsequent app edits keep rebuilding the gallery despite there being no gallery page.

8. **SHOULD-FIX — Resetting the generation breaks SSE reconnection.** [host/web-js/dev-gallery.mjs:126](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:126)  
   A page surviving a connection loss retains its original client `seq`. After idle resets the server counter, rebuilding starts at 1; the client ignores reloads until that counter exceeds its old value. Changes made while disconnected therefore remain invisible. Keep generations monotonic or introduce an explicit epoch.

9. **SHOULD-FIX — Reopening during idle shutdown can lose the start request.** [host/web-js/dev-gallery.mjs:100](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:100)  
   After `stop()` kills a build, `busy` remains true until its exit is processed. A new page arriving in that interval starts nothing. The old refresh then clears `busy` but suppresses continuation because its `run` changed. The new page can remain on “Building the gallery…” indefinitely. Preserve pending demand across shutdown.

10. **SHOULD-FIX — Filesystem failures escape gallery error reporting.** [host/web-js/dev-gallery.mjs:91](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.mjs:91)  
    `refresh()` has `finally` but no `catch`, and callers discard its promise. Failures copying source, removing staging files, or publishing with `replaceBuild()` become unhandled rejections instead of gallery error frames. Catch these within the run and report them through `fail()`.

11. **SHOULD-FIX — The cleanup test masks the behavior it claims to verify.** [host/web-js/dev-gallery.test.mjs:118](/tmp/rv-dt-g3r1/host/web-js/dev-gallery.test.mjs:118)  
    Shutdown happens after builds finish, and `loop.stop()` invokes `spawnOwned.stop()`, which itself kills descendants. The subsequent `pgrep` cannot establish that the loop cleaned them up; matching only `/live` also misses the generator and Git children. Exercise shutdown during both stages, signal only the loop, and inspect recorded children before the cleanup backstop. Error isolation likewise needs an actual failing build.

The wire validation/listing and 1.15 documentation look correct. Gate placement is correct, and the default app client template expands identically to HEAD~1. No builds or test suites were run; nothing was changed.
