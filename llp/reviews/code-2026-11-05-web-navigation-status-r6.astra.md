# Code review: navigation HTTP status, round 6, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec resume` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `91157b40f9` (parent `21c5dfbdf6`).
- **Method:** one follow-up brief (sha256 `8c6fb83a4a6c5949ff3d1a5ea7266fffc56c4fd5588db578b73d6f0431bee759`), the round-5 reviewer resumed, four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

One **P1 blocker** remains in `91157b40f9` relative to `21c5dfbdf6`. The round-5 unowned-work correction is accepted, but **not all earlier blocking settlements are complete**: outstanding I/O can still disappear from server settlement when its source answers.

**P1 — Owned fetches that outlive their source’s answer are omitted from settlement. Incomplete repair of the existing status defect; not introduced by the round-6 delta.**

[held.js:20](/tmp/exact-web-status-review-OEBQRl/checkout-r6/host/web-js/held.js:20) considers tracked answer Promises and unowned work. A fetch started during a source’s call belongs to that call instead. If the source returns synchronously, [ts-data.js:458](/tmp/exact-web-status-review-OEBQRl/checkout-r6/host/web-js/ts-data.js:458) returns its value without creating a ticket or registering the remaining I/O.

Reproduction: resource A reads the status store key and controls a conditional 404 head. Resource B does:

```js
fetch(url).then(() => {
  store.set("status", "missing");
}).catch(() => {});
return 0;
```

No timer, argument change or discarded request is needed.

The probe through the actual runtime and TypeScript adapter produced:

```text
Before release:
  current tickets=[]
  call.settled=true, call.plain=1
  inflight=0, unfinishedAnswers=false, canSetStatus=true
  HTTP 200 committed

After release:
  store reader revalidated
  active head status=404
```

A second form starts an `exactHeld` fetch but returns `Promise.resolve(0)`. Here a ticket exists initially, but [held.js:24](/tmp/exact-web-status-review-OEBQRl/checkout-r6/host/web-js/held.js:24) removes it from `Answers` when that answer Promise resolves, despite `call.held=1`. This also committed 200 before the later 404. Zero-deadline probes returned 200 instead of 503.

**Settlement:** preserve server accounting for outstanding I/O after its owning source returns or its answer Promise settles. Extend the compiled detached-fetch fixture with a directly started ordinary fetch followed by synchronous return, and a fetch outliving an already resolved answer Promise. Require 404 after release and 503/unsettled while the response remains held at the deadline.

The repaired unowned cases passed my probes: both the plain continuation beside a canceled timer and the detached unowned fetch withheld headers, returned 404 after release, and returned 503 at a zero deadline. The new deadline controls provide meaningful causal evidence; the earlier browser-only control that passed does not.

The previously accepted navigation ownership, outer-root exclusion, scoped IDs, constant regions, nested tabs, store/watch dependencies and checkpoint-recovery changes remain accepted. I found no additional compiler or navigation finding. The open-stream and public `serve` `Retry-After` deferrals remain sound for this scope.

The simplification pass favors tracking I/O for its actual lifetime while preserving the separate agent-clock rules. The writing pass found no separate issue. No Contract syntax change is needed.

Verification used Bun 1.4.2 and read-only probes with controlled fetch responses. Application wiring was compiler-equivalent; the render wait reproduced the generated calculation. The unknown-capability regression and `git diff --check` passed. Supplied broader logs report 69 passing tests.

I did **not** run compiled browser fixtures, Rust checks, bundler measurements or the fresh document/build checks. The previously recorded native, Firefox and caps verification limits remain. The working tree is unchanged.