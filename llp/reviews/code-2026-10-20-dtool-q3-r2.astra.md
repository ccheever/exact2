# Code review: the wasm client shell activates within a second without frames, once (LLP 1122 queue item (3)), round 2 of 2, 2026-10-20 (astra)

- **Family:** OpenAI. GPT-6 Astra via codex exec, reasoning effort max, read-only sandbox, a throwaway worktree at `99e5216089`.
- **Method:** one brief (sha256 `a6bfdf850bba789237544d8cc2f402a4dba3b0e125a9b660ec6476e0e71a105a`), shared by both reviewers, each blind to the other's review; commits 1b12a9079a (round 1's) and 99e5216089 (its fixes). The author (Claude) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Verdict:** nothing blocks (1 SHOULD-FIX, 1 NIT).
- **Disposition:** all taken post-round, in the commit named in the sign-off request:
  - Served and `plan ready:` are each latched as their chunk arrives, before the 8 KB tail is cut, and a first compile's `error:` line ends the wait at once with the tail (1).
  - The temporary dist is removed on every path: the dev loop and the browser start inside the `try` (2).
  - Not taken: reading `/exact.json` instead of the log. The marker's wording changing fails closed (no page opened, the tail shown), never a wrong count.

**Nothing blocks.** I found no new correctness defect in `glue.js`. Two test issues remain:

1. **SHOULD-FIX — Readiness depends on transient log text.** [activation-noframes.test.mjs:41](/tmp/rv-dt-q3b/host/web/tests/activation-noframes.test.mjs:41) retains only 8,000 characters, while line 44 requires both the listening URL and `plan ready:` to remain present. If the resident compiler emits more than 8 KB of diagnostics after the banner, the URL disappears; compilation can succeed, yet the test waits ten minutes. The plan marker is [printed only once](/tmp/rv-dt-q3b/host/web/dev.mjs:569), so lost markers need not return.

   A first compilation error also leaves the dev loop alive ([dev.mjs:572](/tmp/rv-dt-q3b/host/web/dev.mjs:572)); `exitCode` cannot detect it, and the same ten-minute wait ends with the misleading “never served.” Changing the marker’s wording likewise causes a timeout despite successful startup. Latch milestones independently of the diagnostic tail and handle compilation failure explicitly; the existing `/exact.json` response also exposes persistent generation readiness.

2. **NIT — Startup failures bypass temporary-directory cleanup.** The cleanup guard starts at [activation-noframes.test.mjs:46](/tmp/rv-dt-q3b/host/web/tests/activation-noframes.test.mjs:46), after readiness and Chromium launch. A readiness timeout or unavailable Chromium executable leaves `exact-noframes-dist-*` behind. `afterEach(stopAll)` stops processes but does not remove that directory. Include startup in the cleanup scope.

The round 1 dispositions otherwise hold:

- **Astra 1 / Grok 1:** The normal startup race is fixed: waiting for publication makes the page adopt the first generation instead of receiving it as a second activation. Finding 1 above concerns the replacement wait’s failure modes.
- **Astra 2:** [agent.test.mjs:891](/tmp/rv-dt-q3b/host/web/agent.test.mjs:891) correctly handles timer-first activation. `early` is a Promise; skipping the empty-`loads` assertion is necessary because activation synchronously starts imports. Promise identity is checked, and the optional-module cases loop over both orders and assert one activation and one input-glue load.
- **Astra 3:** [QUEUE.md:1570](/tmp/rv-dt-q3b/QUEUE.md:1570) now accurately records Canvas 2D’s remaining readiness stall.
- **Grok 2:** Rightly rejected. DOM’s “change an attribute” always invokes “handle attribute changes,” whose first step queues an attribute mutation record; neither checks whether the values differ. Two identical assignments therefore produce two records, which the observer counts separately. [DOM standard](https://dom.spec.whatwg.org/#concept-element-attributes-change)

At [glue.js:1492](/tmp/rv-dt-q3b/host/web/glue.js:1492), the guard retains the same Promise through settlement; served pages still activate immediately, agent mode schedules no fallback timer, and only the frame callback stamps `frameCallbackMs`.

Read-only review; no builds or test suites run.
