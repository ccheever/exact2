# Code review: navigation HTTP status, round 3, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `5a40a2ea79` (parent `21c5dfbdf6`).
- **Method:** one brief (sha256 `4db5ee20235b6ac4a55e47a58b00193c11bbd03a12fe25cd3ef39bc584c881ad`), one reviewer, four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

Four blockers remain in `5a40a2ea79` relative to `21c5dfbdf6`. **I do not accept all earlier blocking settlements.** The exact earlier examples are addressed, but the broader fixes remain incomplete.

1. **P1 — Discarded held requests can still change status after HTTP 200 commits. Incomplete repair of the earlier outstanding-work finding.**

   [document.js:48](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/document.js:48) relies on `inflight`, current tickets and readiness. However, [held.js:29](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/held.js:29) deliberately excludes `exactHeld` fetches from `inflight`. Once such a request loses its resource ticket, all three checks miss it. Its Promise continuation can still write the store.

   Reproduction: use the earlier discarded-request fixture, with the original fetch marked `exactHeld: true`. Initialization changes its arguments, and its replacement answers synchronously. Releasing the original fetch writes `"missing"` to the status key.

   The actual runtime and `pageServer` probe produced:

   ```text
   Boot: current tickets=[], inflight=0, canSetStatus=false
   HTTP before release: 200
   After release: store reader revalidated; active status=404
   ```

   [build.mjs:570](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/build.mjs:570) uses the same incomplete accounting for settlement.

   **Settlement:** distinguish clock-wait accounting from unfinished continuations that can invalidate status. Cover discarded held fetches in both header eligibility and settlement. Add a compiled regression using the actual `exactHeld` API.

2. **P1 — A non-panel ID can change panel selection without receiving a status guard. Incomplete repair of navigation naming.**

   [answers.rs:410](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/src/answers.rs:410) treats `id` as relevant only on possible tabpanels. But [navigation.js:18](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web/navigation.js:18) finds the **first descendant with the matching ID**, then checks its role and ownership. An ordinary view can therefore shadow a panel.

   Reproduction: a fixed tablist names `status-panel` and `plain-panel`. The former contains route `"second"` with status 404; the latter contains the selected `"first"` route. Before both panels, put an ordinary view whose ID changes from `"unused"` to `"status-panel"` when data arrives.

   The probe committed 200 with `canSetStatus=false`. After release, panel lookup dropped `status-panel`, and its previously excluded head became active with status 404.

   **Settlement:** track every ID candidate that can affect the actual lookup, including structural insertion/removal and candidates across navigation ownership boundaries. Add computed-ID and conditional/repeated shadowing regressions. Limiting guards to elements already recognized as panels is insufficient.

3. **P2 — Constant regions around fixed tabs still delay a provably status-free page. Introduced latency regression; incomplete settlement of the earlier inactive-panel finding.**

   [answers.rs:329](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/src/answers.rs:329) scans only direct `Site::Node` children of a tablist. It skips constant regions, although the proof already traverses constant regions when finding selected routes.

   Take the added `conditionalInactiveTab` fixture and wrap its two fixed tab buttons in `if true`. This changes neither their rendered placement nor selection. Nevertheless, the compiler loses the fixed-panel proof and retains the pending condition’s guard.

   The runtime probe returned `canSetStatus=true` before release. After release, the active head remained `"First"` with no status; the conditional second panel was always unselectable.

   **Settlement:** traverse selected constant-region arms consistently when proving fixed tab naming. Require the browser’s headers before releasing this variant’s data.

4. **P2 — The new discarded-work deadline result cannot reliably finish in the browser. Incomplete deadline/client fix.**

   [render.mjs:223](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/render.mjs:223) now recognizes `out.outstanding`, but [activation inference:54](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/render.mjs:54) and checkpoint construction still use the old pending-resource representation.

   With discarded work outstanding, no current tickets, no due timers and an otherwise ordinary inferred page, the actual renderer returned:

   ```text
   status=503, settled=false, activate="interaction", checkpoint.pending=[]
   ```

   There is also a recovery problem beyond activation. The discarded-request fixture’s checkpoint retains all current answers as settled. Feeding that checkpoint to a fresh runtime invoked **zero data sources** and retained `"Checking"`: [rt.js:1444](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/rt.js:1444) restores those answers, without representing the unfinished discarded work.

   **Settlement:** propagate unfinished work into activation and checkpoint recovery. Ensure the browser restarts the necessary work—or conservatively boots afresh—and completes without interaction. The added deadline test checks only server status and `settled`; its explicit `activate=never` cannot establish this behavior.

A separate **P2 existing defect, non-blocking for this patch**, is exposed to newly buffered navigations: [build.mjs:579](/tmp/exact-web-status-review-OEBQRl/checkout-r3/host/web-js/build.mjs:579) classifies every resource ticket as pending, including an open stream that has delivered its first answer. My runtime probe had `pending(resource)=false`, `inflight=0` and active status 404 after the first message, but retained its stream ticket; the renderer consequently returned 503. Settlement should distinguish an unanswered resource from an open stream, with a regression that receives the first answer while keeping the stream open.

The ordinary discarded-request, readiness, inactive-panel structural-change and unrelated-outer-root controls behaved correctly. I accept those specific repairs, and accept keeping the public `serve` adapter’s `Retry-After` issue separately recorded in `QUEUE.md`.

The simplification pass favors one consistent settlement definition and one traversal of constant-region children. The writing pass found no separate blocker. None of the proposed settlements requires public Contract syntax changes.

Verification used Bun 1.4.2 and read-only probes of repository runtime, navigation, renderer and checkpoint code. The unknown-status-capability regression passed; removing its guard in memory made it fail. The added tests assert observable status, header timing, cache headers and client updates. Supplied negative-control logs provide additional evidence, although the older `astra-before.log` fails during parsing and is not behavioral regression evidence.

I did **not** run compiled Contract/browser fixtures, Rust checks or Rolldown measurements. Compiler placement was checked from source; runtime fixtures used compiler-equivalent wiring. In particular, the held-request probe supplied the metadata produced by `ts-fetch.js`, rather than running a compiled TypeScript app. The supplied broader verification logs are not uniformly green. `git diff --check` passed, and the working tree remains unchanged.