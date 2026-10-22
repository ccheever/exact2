# Code review: the variant gallery on the JS dev loop, G3, round 2 of 3 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `34ea4f0c7b5535fa25e34e05b2952ef78d2d0dd1c61d7fd57ce4b41ac71c4548`), shared by both reviewers, each blind to the other's review; HEAD `232dc81055`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 2 BLOCKER, 4 SHOULD-FIX, 1 NIT.
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

This round still has blockers. The `page: 'gallery'` wire field itself looks correct.

1. **BLOCKER — Gallery work starts before the app publishes its batch.** [host/web-js/dev.mjs:494](/tmp/rv-dt-g3r2/host/web-js/dev.mjs:494)  
   `designKept()` immediately enters `refresh()` and spawns the generator before `push(batch)` at line 499. The kept-build and restore hooks have the same ordering. This puts gallery startup on the app’s edit path, contrary to the owner’s condition. Notify after publication, while retaining notification for commits needing no additional batch.

2. **BLOCKER — Gallery inspection starts processes outside its lifecycle cleanup.** [host/web-js/dev-gallery.mjs:182](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:182)  
   The delegated [inspect-table builder](/tmp/rv-dt-g3r2/host/web-js/dev.mjs:377) spawns an untracked child. Neither `gallery.stop()` nor the loop’s shutdown tracks or kills it. Interrupting the loop during gallery inspection can leave that process computing and writing its output after the loop exits; idle cleanup also misses it. Inspection work needs lifecycle ownership too.

3. **SHOULD-FIX — Cancelling an unchanged scrub can rebuild the gallery.** [host/web-js/dev-gallery.mjs:217](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:217)  
   The remembered design starts as `null` and is updated only by `designKept()`. Initial builds and `appChanged()` never seed it. Consequently, the first scrub restore calls `designKept(currentDigest)` and rebuilds despite no committed edit. The same happens after a full reload changes the design. Track the digest at initial and full publications.

4. **SHOULD-FIX — Opening the first page queues an unnecessary second build.** [host/web-js/dev-gallery.mjs:139](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:139)  
   The HTML request starts a build; its waiting page then opens SSE. Since `built` is still false and `busy` is true, that request sets `again`, causing another complete build and reload without an app edit. Preserve demand arriving during shutdown without treating requests joining the current build as new work.

5. **SHOULD-FIX — B6’s shared Cargo-lock contention remains on fallback.** [host/web-js/dev-gallery.mjs:87](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:87)  
   When `contract` is absent or returns exit code 2, the gallery still runs Cargo in the app’s workspace with the same target directory. This can occur while the design server is starting and can delay subsequent app/compiler work. Direct binaries fix the warm path, but the fallback still violates the isolation condition.

6. **SHOULD-FIX — The cleanup test still cannot establish “nothing left.”** [host/web-js/dev-gallery.test.mjs:129](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.test.mjs:129)  
   Signalling only the loop before the backstop is an improvement. However, filtering command lines by the cache path excludes descendants such as `git status` and `git rev-parse`. The test samples whichever stage happens to be running, rather than exercising both stages; idle cleanup asserts only a log message. Record descendants/groups and check them before the backstop in each lifecycle case.

7. **NIT — B12’s fallback still misses the builder’s headless HTML.** [host/web-js/dev-gallery.mjs:207](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:207)  
   If the exact base-tag match misses, the fallback requires an explicit `<head>`. The [builder’s shell](/tmp/rv-dt-g3r2/host/web-js/build.mjs:638) omits that tag. Thus the fallback can still insert nothing. Today’s exact match works.

Round 1 disposition, with A/B referring to the supplied reviews:

| Items | Status at HEAD |
|---|---|
| A1, B5 | **Closed:** independent gallery map retention at [dev-gallery.mjs:119](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:119). |
| A2 | **Closed for routing:** gallery table at [dev-gallery.mjs:179](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:179), relative fetch at [inspect-dev.js:27](/tmp/rv-dt-g3r2/host/web-js/inspect-dev.js:27). Lifecycle issue is finding 2. |
| A3, B7 | **Missed updates fixed:** [dev.mjs:494](/tmp/rv-dt-g3r2/host/web-js/dev.mjs:494) and [dev.mjs:842](/tmp/rv-dt-g3r2/host/web-js/dev.mjs:842). New hook defects are findings 1 and 3. |
| A4 | **Closed for build groups:** ownership retained until the group disappears at [dev-gallery.mjs:69](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:69). |
| A5, B1 | **Closed:** assets served through the app’s safe build tree at [dev-gallery.mjs:190](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:190). |
| A6, B2 | **Closed:** request retry at [dev-gallery.mjs:139](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:139); edit retry uses activation at [dev-gallery.mjs:214](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:214). |
| A7, B4 | **Closed:** requests rearm quiet cleanup at [dev-gallery.mjs:129](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:129). |
| A8 | **Closed:** stop preserves `seq` at [dev-gallery.mjs:145](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:145). |
| A9 | **Closed for lost demand:** continuation at [dev-gallery.mjs:114](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:114). Finding 4 covers excess demand. |
| A10, B8 | **Closed:** refresh catches failures at [dev-gallery.mjs:109](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:109). |
| A11, B11 | **Partial:** asset/error coverage added; descendant cleanup remains finding 6. The wasm route remains untested here. |
| B3 | **Closed:** bounded wait and child-kill fallback at [dev-gallery.mjs:148](/tmp/rv-dt-g3r2/host/web-js/dev-gallery.mjs:148). |
| B6 | **Partial:** finding 5. |
| B9 | **Closed:** both URL spellings at [host/web/dev.mjs:1340](/tmp/rv-dt-g3r2/host/web/dev.mjs:1340). |
| B10 | **Closed:** corrected module path at [tamber-builds.mjs:12](/tmp/rv-dt-g3r2/scripts/tamber-builds.mjs:12). |
| B12 | **Partial:** finding 7. |

The gate precedes lazy startup, stream delivery stays separate, and wire validation/listing and the 1.15 documentation are correct. The default app client compares byte-for-byte equal to pre-G3.

Nothing was changed. No builds or test suites were run.
