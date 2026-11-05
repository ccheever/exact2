# Code review: navigation HTTP status, round 2, adversarial (astra)

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex exec` 0.166.0, reasoning effort max, read-only sandbox, a throwaway worktree at `5f159e88e7` (parent `148dcff048`).
- **Method:** one brief (sha256 `ffa030d0d985037f632000dc1cb72d4da93a026d96b45262e03b570493ab2135`), one reviewer, four passes: adversarial correctness, simplification, writing and verification. The author (Sol 6.1) is not a reviewer.
- **Transcription:** the final message, unedited; links and commit shas name the review worktree.

Three blockers remain in `5f159e88e7` relative to `148dcff048`. The exact first-round examples are repaired, but I cannot accept either settlement as complete.

1. **P1 — Outstanding work with a discarded ticket can still change the status after HTTP 200 commits. Incomplete repair of first-round finding 1.**

   [document.js:48](/tmp/exact-web-status-review-OEBQRl/checkout-r2/host/web-js/document.js:48) checks current resource tickets and readiness flags. A request whose ticket was discarded can still be running, counted in `inflight`, and capable of writing the store.

   Reproduction: a synchronous resource reads the status key. Another resource starts an asynchronous request using an initial mode. A synchronous initialization resource changes that mode during boot, causing the asynchronous resource to be re-asked and answer synchronously. Its original request remains outstanding. Release that request and let it write `"missing"` to the status key.

   The actual runtime and `pageServer` probe produced:

   ```text
   Boot: all current resource tickets absent; inflight=1; canSetStatus=false
   HTTP response: 200 committed
   Released request: store reader revalidated; active head status=404
   ```

   **Settlement:** include outstanding work that can invalidate resources even after its ticket is discarded. Use the runtime’s outstanding-work accounting alongside readiness, and add a compiled regression with the discarded request held until explicitly released.

2. **P1 — Selection changes inside an inactive tabpanel are ignored. Incomplete repair of the overall status defect.**

   [regions.rs:81](/tmp/exact-web-status-review-OEBQRl/checkout-r2/host/web-js/src/regions.rs:81) emits structural guards without selection ownership. [document.js:66](/tmp/exact-web-status-review-OEBQRl/checkout-r2/host/web-js/document.js:66) then rejects guards beneath currently inactive panels, even when their changes can alter the enclosing navigation’s selection.

   I reproduced this with both conditional and repeated routes that introduce the selected key into an earlier tabpanel.

   A separate reproduction needs no duplicate route keys: panel `a` contains route `"second"` with `head status=404`; panel `b` contains the selected route `"first"`. A tablist after the panels names both. Inside inactive panel `a`, a pending condition introduces an earlier tablist naming only `a`. When it mounts, `panelsOf` chooses that list; no named panel contains `"first"`, so `unselectedForHead` exposes the status head. The condition’s guard was ignored at boot.

   All three probes committed HTTP **200** before release and settled with an active **404**.

   **Settlement:** associate structural and naming guards with the navigation selection they can change. Current inactivity alone cannot exclude them. Cover conditional and repeated panel contents and tab naming inside inactive panels, while preserving exclusion by unrelated outer navigation.

3. **P2 — A provably inactive conditional tabpanel still delays headers. The regression from first-round finding 2 remains in this form.**

   [answers.rs:187](/tmp/exact-web-status-review-OEBQRl/checkout-r2/host/web-js/src/answers.rs:187) disables the literal-route exclusion whenever tabs are possible, including completely fixed tab arrangements.

   Reproduction: the navigation key is literal `"first"`; a fixed tablist names panels `a` and `b`; always-present panel `a` contains `"first"` and an ordinary head. A pending condition mounts panel `b`, containing only literal route `"second"` and `head status=404`.

   That head is never selectable. Nevertheless, the probe returned `canSetStatus=true` and withheld the response until release. The active head remained unchanged. If the data reaches the deadline, buffering also exposes the page to the renderer’s 503 response.

   **Settlement:** extend the exclusion proof to fixed panel naming and an always-present selected route. Require headers before releasing this fixture’s data.

The simplification pass favors tracking the affected navigation owner and actual outstanding work over adding more positional exceptions. The writing pass found no separate blocker. I accept keeping the pre-existing public `serve` `Retry-After` adapter issue separately recorded in `QUEUE.md`.

Verification used Bun 1.4.2. The repository’s unknown-status-capability test passed. Read-only probes exercised the actual runtime, navigation helpers and `pageServer`; renderer controls confirmed status precedence, buffered-error cache headers, 503 `Retry-After`, and successful-page caching. Both diffs passed `git diff --check`.

The runtime fixtures used hand-wired compiler-equivalent calls; compiler placement was checked from source. I did **not** run compiled Contract/browser fixtures, Rust checks or Rolldown/gzip measurements. The renderer controls supplied pending results rather than testing wall-clock deadline enforcement. The working tree remains unchanged.