# Code review round 3: exactBodyFrom (9dca6be37), astra

Reviewer: GPT-6 Astra (codex, max reasoning, read-only), 2026-10-08, a confirming round run by the lead on 9dca6be37.

1. **Blocker — native file reads still outlive cancellation and timeout.** [executor_core.rs:932](/private/tmp/bsky4-rv/wt-bf/host/apple/src/executor_core.rs:932), [executor_body.rs:116](/private/tmp/bsky4-rv/wt-bf/host/apple/src/executor_body.rs:116). The shared iOS/macOS/Linux executor waits synchronously for `read_capped`, which never observes cancellation. A stalled filesystem read keeps the request and worker pending indefinitely despite `exactTimeout`. Round-2 finding 2 remains unfixed. **Fix:** make acquisition cancellation-aware and permit timeout settlement without waiting for the blocked read; discard late results.

2. **Blocker — both web paths can still send after expiry.** [http-body.js:197](/private/tmp/bsky4-rv/wt-bf/host/web/http-body.js:197), [ts-fetch.js:58](/private/tmp/bsky4-rv/wt-bf/host/web-js/ts-fetch.js:58). The wasm timestamp starts *after* filesystem-grant validation, although its timer starts before it. The JS target checks elapsed time before calling `fetchWith`, which performs further synchronous grant validation before sending. Neither accounts for this work when the timer callback is delayed. With 1,000 valid grants, probes reached fetch approximately **9 ms into a 1 ms deadline** on wasm and **31 ms into a 15 ms deadline** on the JS target. **Fix:** carry one absolute monotonic deadline through acquisition and admission, check it immediately before transport, and retain its cancellation signal.

3. **Should-fix — streaming uploads ignore the caller’s signal.** [ts-stream.js:24](/private/tmp/bsky4-rv/wt-bf/host/web-js/ts-stream.js:24), [ts-stream.js:37](/private/tmp/bsky4-rv/wt-bf/host/web-js/ts-stream.js:37). URL, method and headers now survive a `Request`, but its signal—and `init.signal`—is discarded. Both pre-aborted cases still read and send the file in probes. **Fix:** select the caller’s signal using Fetch’s override rules and combine it with the stream controller before acquisition.

4. **Nit — the prelude’s UTF-8 cap still mishandles lone surrogates.** [prelude.js:654](/private/tmp/bsky4-rv/wt-bf/js/src/prelude.js:654). `"app:/data/" + "\ud800".repeat(1500)` passes despite encoding to 4,510 bytes with `TextEncoder`. **Fix:** reject malformed Unicode or implement matching UTF-8 counting.

The compat requirement now rejects old-shell cohorts. The wasm pre-abort check, stream URL/header inheritance, pinned roots, scoped grants, byte cap, worker path forwarding, Windows refusal and Rust-module-seam refusal check out by source tracing and in-memory probes. No files changed; no native builds ran.

**Blocks landing: yes — findings 1 and 2.**