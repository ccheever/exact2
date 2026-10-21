# Code review: the JS target's first-size and geometry boot races, round 1 of 2 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree.
- **Method:** one brief (sha256 `10a88642c4191f27039bb664aa070dc225e112ad5690afb02663c5765508b7aa`), shared by both reviewers, each blind to the other's review; HEAD `ed8faf130b`. The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (4 SHOULD-FIX).
- **Disposition:**
  Nothing blocks from either reviewer. All taken, in the round 1 commit:
  - Astra 1 (a view removed, or an action that throws, kept the hold until the bound): the hold is let go with the view's scope (`onEnd`), and after the action whatever it does (`finally`). The bound's timer is cleared on release.
  - Astra 2, Grok 2 (scheduling-dependent tests): the reports are held until the page itself says so, 300 ms into the settle (in the same evaluate, no driver gap). The geometry module is served only then, and the state is read in the same evaluate as the settle. Checked by mutation: main fails (widths []); the fix without the geometry chain fails (zeros); the fix without the no-box release fails (2.1 s).
  - Astra 3, Grok 1 (no-box release untested): the box with no layout is never reported in the test, and the settle must answer under 1.5 s, well inside the 2 s bound.
  - Astra 4 (the wasm host has the same races): QUEUE'd with the glue.js lines. Conform's reference won the race in every recorded run, and this change is the JS side's.

**Nothing blocks this JS fix.** Both diagnosed races are real. I found these follow-ups:

1. **SHOULD-FIX — [observers.js:20](/tmp/rv-dt-rb1/host/web-js/observers.js:20): removal and callback errors retain the hold until timeout.** If an element disappears after attachment, [resize-glue.js:12](/tmp/rv-dt-rb1/host/web/resize-glue.js:12) discards its report without calling `landed()`. Settle unnecessarily waits two seconds. A thrown `f()` similarly skips `landed()`; the promise’s `.catch()` does not catch asynchronous observer callbacks. Release on scope teardown and in a callback `finally`, and clear the fallback timer when released.

2. **SHOULD-FIX — [resize-boot.test.mjs:65](/tmp/rv-dt-rb1/host/web-js/resize-boot.test.mjs:65): the regression tests depend on scheduling.** The 400 ms report delay can expire before the agent starts settling; the 800 ms geometry delay does not guarantee resize runs first without the fix. Moreover, reading state in a separate `page.evaluate()` allows a late callback to repair state after settle already returned. These cases can pass broken code under load. Use controlled delivery gates and capture state alongside the settle result.

3. **SHOULD-FIX — [resize-boot.test.mjs:87](/tmp/rv-dt-rb1/host/web-js/resize-boot.test.mjs:87): the no-box assertion does not test immediate release.** `hidden <= 1` and elapsed time below ten seconds both pass if the hidden element unnecessarily waits for the full two-second fallback. Suppress its reports explicitly and assert its hold is released independently of that fallback.

4. **SHOULD-FIX, pre-existing — [glue.js:558](/tmp/rv-dt-rb1/host/web/glue.js:558): the wasm oracle remains susceptible to both races.** Resize attachment/reporting is untracked, and geometry loads independently at [glue.js:1440](/tmp/rv-dt-rb1/host/web/glue.js:1440). Wasm can therefore settle before reporting, or dispatch a resize action while geometry is unavailable. Conform opening wasm first gives it extra time but provides no ordering guarantee; fixing JS alone cannot guarantee stable comparisons.

Otherwise, the accounting is sound: each registration owns an idempotent `landed()`, so sharing `Resizes` and the observer cannot double-decrement. Setup failures release their holds. The two-second bound starts **after attachment**, not before module loading; stalled imports can exhaust settle’s existing twenty-second deadline.

The geometry chain introduces no cycle. Generated code calls `geo()` synchronously after mounting, before the deferred `geoLoading()` lookup. Pages without geometry receive `null`; geometry failures are logged and allow resize attachment to proceed, with frames still unavailable.

`__exactRender` remains bypassed. With the agent off, geometry-dependent pages still delay resize attachment until geometry finishes—an intentional release behavior change. Normal reports release promptly; the layout read and uncancelled timers add work but impose no universal two-second delay.

Read-only inspection only; no builds or tests run.
