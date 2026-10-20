# Code review: LLP 1122 T1 commit 2 (the design tool's skeleton and its connection), round 3, 2026-10-15 (astra)

- **Family:** OpenAI. `gpt-6-astra` via the Codex CLI, reasoning effort max, read-only sandbox, a throwaway worktree at `a44b4e3a5` (commit `0beef42fd` and its round-1 and round-2 fixes).
- **Method:** one brief (sha256 `7ffbbbea1ef3129db43e3f6b5c6db6b4d266675de45f032ab18940edd1e62c0c`), shared by both reviewers, blind to the other review. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** request changes (1 medium, 2 low).
- **Disposition:** all taken: reuse compares the served page, `app.js` and `app.plan` with this checkout's build, so a build differing only in its module is refused; Ctrl-C (or SIGTERM, SIGHUP) while the design server starts closes it and exits 130, 143 or 129; Try Again after an answered poll moves the cursor on first, as `advance` would. Round 3 is the last: no test was added for two builds with the same page, or for Try Again between an answer and its re-ask.

---

Read-only review; no builds or tests were run. Three issues remain:

1. **Medium — Reuse still accepts stale client code.** [scripts/tamber-server.mjs:737](/tmp/rv-dt-2r3/scripts/tamber-server.mjs:737)  
   Comparing `index.html` does not identify the tool build: the generated HTML references the unversioned `./app.js`. Changes to `app.ts` or Contract actions can leave that HTML identical. A server from checkout A can therefore pass checkout B’s comparison while serving older client code, leaving round 2’s finding unresolved.  
   **Fix:** compare a build fingerprint covering the executable artifacts. Test two builds with identical HTML and different JavaScript.

2. **Low — Startup cancellation bypasses child cleanup.** [scripts/tamber-server.mjs:764](/tmp/rv-dt-2r3/scripts/tamber-server.mjs:764)  
   Moving `await tamber.ready` before `listen()` also moves it before installation of signal handlers and launcher-death monitoring. During a cold build—or while Cargo waits for a build lock—Ctrl-C exits the CLI without calling `tamber.close()`. The detached Cargo process group receives no forwarded signal and can continue running.  
   **Fix:** install child cleanup and launcher monitoring immediately after spawning the child, independently of when the HTTP port is bound.

3. **Low — Try Again can replay an already-consumed batch.** [apps/design-tool/app.contract:64](/tmp/rv-dt-2r3/apps/design-tool/app.contract:64)  
   The pending guard prevents overlapping polls, but `reconnect` increments `asks` without advancing `since` and `epoch`. If hello has failed and Try Again runs after a successful poll completes but before its `after(1)` action, the next request uses the previous cursor. `waitChanges` prepends those same events again, producing duplicate rows and keys.  
   **Fix:** copy `changes.seq` and `changes.epoch` before re-asking, or share the cursor-advancement action. Cover reconnect between poll completion and timer advancement.

**Verdict: Request changes.**
