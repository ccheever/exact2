# Code review: the wasm host's first-size boot races, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `a684902ea4794fbff2e8b2a2b4b51deee9789435432fb0e68714fa50ee3b6f2b`), shared by both reviewers, each blind to the other's review; HEAD `abc0fb0df3`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** 1 BLOCKER, 2 SHOULD-FIX
- **Disposition:**
  All findings taken in the round 1 commit:
  - Astra 1 (BLOCKER: a module replacement drains `inflight` through `httpHelpers()`, which waits for `moduleReady`, so a pending first-size hold stalls activation until the deadline): the hold moves to `mediaAttaching`. A settle waits on that set (`mediaAttached` and the settle loop); a module replacement does not, as for the media's attaches.
  - Astra 2 (a reload clears the set without releasing a waiter that already holds the promise): the reload's teardown calls `el.exactResize?.()` for each view.
  - Astra 3 (the geometry case's 1.5 s fallback could pass without the fix): geometry is now released once the resize glue has run (`exact.observeResize`) and a frame has passed, plus two frames if the tab row was observed. There is no time fallback; if the glue never runs, the test fails.
  - Grok 1 (NIT: a dist built without EXACT_WEB_LINK=all imports no geometry and fails like the race): the prerequisite reads the bake receipt's EXACT_WEB_LINK, as pan-release-engine.test.mjs does.
  Rechecked by mutation: main fails (2 ms settle), no geometry chain fails (widths), no no-box release fails (settled: false).

**One blocker found.**

1. **BLOCKER — [host/web/glue.js:558](/tmp/rv-dt-wrb1-astra/host/web/glue.js:558): resize holds can prevent initial module activation.** When the dev generation replaces the baked module and a first-size hold remains pending, activation calls `boot(..., module)` at line 1480. That calls `waitForInflight`, whose `httpHelpers()` waits for `moduleReady`. But `moduleReady` resolves only after activation finishes. Even if the resize subsequently lands, the helper wait remains blocked: after 20 seconds, replacement throws and `activateData()` is skipped. The resize wait must work independently of module activation.

2. **SHOULD-FIX — [host/web/glue.js:1348](/tmp/rv-dt-wrb1-astra/host/web/glue.js:1348): reload clears membership without releasing existing waiters.** Reload teardown never calls `el.exactResize?.()`. An ongoing `waitForInflight` has already captured the old promise in `Promise.race(now)`; `inflight.clear()` cannot wake it. Removing the element suppresses its resize delivery, so that wait unnecessarily lasts until the remaining two-second bound—or the 20-second settle deadline if module loading has not completed. Resolve resize holds before clearing the old views. A settle started *after* reload does not inherit those cleared holds.

3. **SHOULD-FIX — [host/web/tests/resize-boot.test.mjs:149](/tmp/rv-dt-wrb1-astra/host/web/tests/resize-boot.test.mjs:149): the geometry regression check can pass without the geometry wait.** If resize-glue loading or observer attachment takes longer than 1.5 seconds, the fallback opens geometry before any report, including with the fix removed. The resulting widths are correct and the regression passes. Synchronize the negative case with actual observer/report readiness; use a timeout to fail an inconclusive setup, rather than release the gate successfully.

The core ordering is otherwise sound: `geometryLoading` precedes binding and survives reload; artifacts without `exact_geometry` gain no geometry load. First delivery, dispatch exceptions, load rejection, and both destroy sites release their individual holds. Shared `resizes` does not share those holds. The no-box check and two-second timer run **after module loading**, so two seconds is not an overall hold bound.

Ticketless promises are already supported by `waiting()`. Non-settle replies can now count resize holds; I found no runner or test dependency requiring ticket-only counts, although the driver’s “requests … under pending” diagnostic becomes misleading.

Read-only review; no builds or tests run.
astra exit 0
