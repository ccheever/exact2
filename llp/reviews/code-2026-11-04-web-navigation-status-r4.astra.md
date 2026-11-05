# Code review: navigation HTTP status, round 4, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `226dcfaf75` (parent `21c5dfbdf6`).
- **Method:** one brief (sha256 `2bedf98ba614116e140d38b1d35ce21c6842f9201fa69f1013631195b0f4442b`), one reviewer, four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

Three blockers remain in `226dcfaf75` relative to `21c5dfbdf6`. **I do not accept all earlier blocking settlements.**

1. **P1 — An ID inside an inactive nested route can still expose an outer status after HTTP 200 commits. Incomplete navigation fix.**

   [document.js:66](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/document.js:66) excludes a guard at any inactive navigation boundary absent from its selections. The compiler records affected status-owning ancestors in [answers.rs:431](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/src/answers.rs:431), but an intervening navigation root with no status descendants is absent.

   Reproduction: an outer root selects `"first"` through `plain-panel`; `status-panel` contains `"second"` with status 404. Before those panels, an inner root selects `"inner-first"`. Inside its inactive `"inner-second"` route, data changes an ordinary view’s ID to `"status-panel"`.

   Outer tab lookup searches all descendants before checking ownership, so this ID shadows the real panel. The guard nevertheless gets excluded by the inner root.

   Actual runtime/navigation/`pageServer` probes produced:

   ```text
   Boot: canSetStatus=false; HTTP 200 committed
   After release: status-panel drops from panel lookup; active status=404
   ```

   Computed IDs, conditional insertion and repeated insertion all reproduced it.

   **Settlement:** assess visibility of the navigation owner whose lookup can change. Inactivity below that owner cannot exclude its ID candidates; unrelated navigation above the owner must still exclude it.

2. **P2 — Abandoned timer-only answers now force a false deadline result. Introduced settlement regression.**

   [held.js:15](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/held.js:15) removes tracked work only when its Promise settles. However, an answer awaiting its own timer can permanently lose that possibility when its ticket is discarded: [ts-fetch.js:195](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/ts-fetch.js:195) forbids its timer from running. [ts-data.js:453](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/ts-data.js:453) explicitly accounts for such answers never settling.

   Reproduction: the old-mode request returns `new Promise(resolve => setTimeout(() => resolve(0), 20))`; initialization changes the mode during boot, and the replacement answers synchronously. The page has a literal active 404.

   Using the actual TypeScript adapter and timer implementation, the probe reached:

   ```text
   current tickets=[]; inflight=0; timer.live()=false
   unfinishedAnswers=true; active head=404
   ```

   The new [outstanding calculation](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/build.mjs:570) remains true until the deadline. Renderer controls then return **503, unsettled**. Removing only the added unfinished-Promise contribution gives **404, settled**. This case has no stream.

   **Settlement:** retire discarded continuations whose only remaining work is a timer that cannot run, while retaining discarded held-fetch and ordinary-I/O continuations. Add a compiled regression requiring the settled status without consuming the deadline.

3. **P2 — Unrelated duplicate IDs disable the fixed-panel proof and delay a provably status-free page. Introduced latency regression; incomplete inactive-panel settlement.**

   [answers.rs:304](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/src/answers.rs:304) builds one global ID map and marks duplicates ambiguous. The fixed-panel proof consults that map at [line 354](/tmp/exact-web-status-review-OEBQRl/checkout-r4/host/web-js/src/answers.rs:354). Runtime tab lookup searches only descendants of the particular navigation root.

   Take the fixed conditional-inactive-panel fixture and add an ordinary node in an unrelated sibling root with the selected panel’s ID. That duplicate cannot affect this root’s lookup, but it destroys the compiler’s proof.

   The compiler-equivalent runtime probe withheld headers until data was released. Afterwards, the conditional second panel remained excluded and the active head still had no status. At the deadline, unnecessary buffering also exposes this page to 503.

   **Settlement:** scope ID resolution and ambiguity to the navigation root’s actual lookup domain, preserving descendant shadows across nested ownership boundaries. Require headers before releasing data in this unrelated-duplicate variant.

I accept the specific store/watch dependency, discarded held-fetch, constant-control traversal, and no-interaction checkpoint-recovery repairs. In particular, an additional probe through the actual `exactHeld` implementation now held headers with no current tickets and subsequently returned 404. The broader navigation, fixed-panel and unfinished-work settlements remain incomplete for the reasons above.

Deferring the answered-open-stream defect and public `serve` adapter’s `Retry-After` loss is sound as a scoped follow-up. Both are existing, separately recorded defects; neither is corrected here. The stream defect still affects newly buffered navigations.

The simplification pass favors matching the actual navigation lookup domain and distinguishing runnable continuations from unresolved Promises. The writing pass found no separate blocker. No public Contract syntax change is needed.

Verification used Bun 1.4.2, read-only runtime probes, and renderer controls. The unknown-capability regression and `git diff --check` passed. The added regressions check observable behavior; the supplied before/after logs and checkpoint-removal control provide meaningful causal evidence.

I did **not** run compiled Contract/browser fixtures, Rust checks or Rolldown measurements locally. Compiler placement was checked from source; navigation fixtures used compiler-equivalent wiring. Renderer controls supplied completion states, and the timer probe reproduced the generated wait calculation. Supplied focused suites pass, but the broader root test job exits 101; Firefox startup and native timing remain verification limits. The working tree is unchanged.