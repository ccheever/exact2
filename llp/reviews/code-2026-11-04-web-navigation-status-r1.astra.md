# Code review: navigation HTTP status, round 1, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `dc4df4f855` (parent `148dcff048`).
- **Method:** one brief (sha256 `6f5de0d379bb6f3d331da7bc911f3981abbb25d1fc67182dd2c9501e2ec2031c`), one reviewer, four passes: adversarial correctness, simplification, writing and verification. The author (Sol 6.1) is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

Two blocking issues remain in `dc4df4f855` relative to `148dcff048`.

1. **P1 — Store-driven revalidation can still send HTTP 200 for a settled 404. Blocking; incomplete repair of the existing defect.**

   [document.js:51](/tmp/exact-web-status-review-OEBQRl/checkout/host/web-js/document.js:51) follows resource tickets, readiness and `src` dependencies. It misses the implicit dependency through the store: [rt.js:387](/tmp/exact-web-status-review-OEBQRl/checkout/host/web-js/rt.js:387) re-asks settled store readers after an asynchronous store write.

   Reproduction: resource A synchronously returns whether a granted store key equals `"missing"`. A conditional `head status=404` reads A. Resource B awaits a held response, then writes `"missing"` to that key. At boot, A is settled and B is pending, but B is outside the guard’s dependency graph.

   A read-only probe using the actual runtime and `pageServer` produced:
   - Boot: `canSetStatus=false`; HTTP 200 committed.
   - After releasing B: A re-evaluated; the active head became 404.

   **Settlement:** account for implicit invalidations of status-relevant resources. Conservatively hold headers when outstanding work could invalidate a store reader that controls the status. Add a compiled Contract regression with the response held until the test releases it. This requires no syntax change.

2. **P2 — A condition that can mount only an inactive status head unnecessarily holds the active page’s headers. Blocking; introduced regression.**

   [regions.rs:17](/tmp/exact-web-status-review-OEBQRl/checkout/host/web-js/src/regions.rs:17) wraps the condition, while [document.js:35](/tmp/exact-web-status-review-OEBQRl/checkout/host/web-js/document.js:35) records its parent’s position. That position loses the navigation ownership of the unmounted descendants.

   With `ready` supplied by pending data:

   ```text
   main navigationBack="" navigationKey="first"
     view navigationKey="first"
       head title="First"
     if ready
       view navigationKey="second"
         head status=404
   ```

   The active route cannot acquire a status: its key and both route keys are fixed. Nevertheless, the probe returned `canSetStatus=true` at boot because the guard belongs to `main`. After settlement, the active head still contained only `"First"`.

   This page now waits for the data or deadline instead of retaining its early head. The added inactive-route test covers an already mounted inactive head, so it misses this case.

   **Settlement:** retain enough navigation ownership to exclude a condition whose status descendants are provably unselectable. Add this case and require headers before releasing its data.

A related **P2 pre-existing issue, non-blocking for this commit**, remains in public `serve`: [serve.mjs:982](/tmp/exact-web-status-review-OEBQRl/checkout/host/web/serve.mjs:982) drops `Retry-After`, and [serve.mjs:918](/tmp/exact-web-status-review-OEBQRl/checkout/host/web/serve.mjs:918) does not restore it. I confirmed that a `pageServer` result containing `503` and `Retry-After: 1` loses the header through `sitePages`. Forward it through both adapter steps and test public `serve`; the new browser fixture directly exposes `pageServer`.

The simplification and writing passes found no additional blocking issue.

Verification used Bun 1.4.2 and read-only, in-memory probes of the repository modules. The unknown-capability regression returned **200 on the parent and 404 on this commit**. Controls for failed-resource conditions, mounted inactive heads and response cache headers behaved as intended. `git diff --check` passed.

I did not run the compiled Contract/browser suite, Rust checks or Rolldown/gzip measurements: this read-only checkout has no build artifacts or installed dependencies. The runtime reproductions used hand-wired compiler-equivalent calls; they do not establish end-to-end compiler verification. The working tree remains unchanged.