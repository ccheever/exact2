# Code review: navigation HTTP status, round 5, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec resume` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `106aa58448` (parent `21c5dfbdf6`).
- **Method:** one follow-up brief (sha256 `d2683fc0c0347188bd3d13c17f8dbf1e4f7671ee1d4f38b6741ab3161ae3aae7`), the round-4 reviewer resumed, four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

One **P1 blocker** remains in `106aa58448` relative to `21c5dfbdf6`. **I do not accept every earlier blocking settlement:** the timer-only case is repaired, but its retirement rule reopens the outstanding-work defect.

**P1 — A discarded answer can retire while an unattributed fetch can still change status. Incomplete status fix; regression introduced since round 4.**

[held.js:18](/tmp/exact-web-status-review-OEBQRl/checkout-r5/host/web-js/held.js:18) checks the answer’s `held`, `plain` and `firing` counters before retiring it. It omits `loose.n`, which tracks work started without a current answer—for example, a fetch inside a plain `.then`. [ts-fetch.js:39](/tmp/exact-web-status-review-OEBQRl/checkout-r5/host/web-js/ts-fetch.js:39) explicitly uses that accounting path.

Use the discarded-request fixture, with its old-mode answer doing:

```js
setTimeout(() => {}, 10000);
return Promise.resolve()
  .then(() => fetch(url, { exactHeld: true }))
  .then(() => {
    store.set("status", "missing");
    return 0;
  });
```

Initialization changes the mode; the replacement answers synchronously. The original timer becomes inactive, but the fetch remains outstanding.

The timer-only ticket is absent from `Candidates`, so [looseFetch](/tmp/exact-web-status-review-OEBQRl/checkout-r5/host/web-js/held.js:52) does not recount it into `inflight`. The new retirement rule then deletes its unfinished answer.

Read-only probes through the actual runtime, TypeScript adapter, fetch/timer implementation and `pageServer` produced:

```text
Before release:
  current tickets=[]
  inflight=0
  call.timers=1, held=0, plain=0, firing=0
  canSetStatus=true, unfinishedAnswers=false
  HTTP 200 committed

After release:
  store reader revalidated
  active head status=404
```

Both ordinary and `exactHeld` fetches reproduced this. An in-memory control adding `!loose.n` to the retirement condition withheld headers and returned 404 after release. A directly started held fetch beside the canceled timer also behaved correctly.

**Settlement:** include unattributed I/O when deciding that a discarded continuation cannot resume. Add a compiled regression that starts the fetch inside `.then`, and require headers to remain pending until release, followed by HTTP 404. Preserve the near/future abandoned-timer controls.

I accept the navigation-owner, unrelated-outer-root, scoped-ID, constant-region and nested-tab ownership settlements. Computed, conditional and repeated nested ID shadows now waited correctly in my probes; unrelated outer roots still excluded them, including a guard located on its owner. Near and future abandoned timers also settled correctly. The prior store/watch dependency and checkpoint-recovery repairs remain accepted, subject to the outstanding-work gap above. I found no additional compiler, cache-header or client-bundle blocker.

The simplification pass reinforces using consistent work accounting for retirement and settlement. The writing pass found no separate issue. No public Contract syntax change is needed. The previously accepted open-stream and public `serve` `Retry-After` deferrals remain sound for this scope.

Verification used Bun 1.4.2. The unknown-capability regression and `git diff --check` passed. Supplied negative controls show observable failures for the repaired navigation, ID and timer cases; the mixed-fetch control that passed establishes no causality for its removed predicate.

I did **not** run compiled Contract/browser suites, Rust checks or Rolldown locally. My application wiring was compiler-equivalent, and its render wait reproduced the generated calculation. Supplied suites report 68 passes; root tests remain non-green. Main/fix controls reproduce the worker failure, and the fix control additionally records a timeout waiting for an aborted write to reach its server. Firefox startup remains a verification limit. The working tree is unchanged.