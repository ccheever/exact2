# Ordered web HTTP reads the whole response before any limit

**Status:** Closed
**Resolution:** Fixed: ordered browser reads default to the native 64 MiB streaming ceiling, cancel oversized streams before wasm fulfillment, and retain independent per-request limits. Bun regressions cover direct reads and cancellation on the 65th 1 MiB chunk.
**Systems:** Web host, Data seam
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1016

Native ordered fetches cap the body at 64 MiB while it is still arriving (`host/apple/src/executor_core.rs`, `MAX_BODY`; Linux uses that file). The web host does not. Contract resources default to ordered (`runner/src/request.rs`). `host/web/src/batch.rs` emits `maxResponseBytes` only for `HttpScheduling::Independent`. `glue.js` then calls `boundedHttpBody(r, op.maxResponseBytes)`. A missing limit means `response.arrayBuffer()` with no ceiling (`host/web/http-body.js`). `fulfill` copies that buffer into wasm through `exact_in`, which resizes the input buffer to the full length (`host/web/src/abi.rs`).

One oversized response from a granted origin allocates on the page thread and grows the wasm heap before the runner can refuse it. Independent requests are checked, and a bad independent limit is refused before `fetch`.

Apply the same 64 MiB ceiling to ordered responses while the body is streaming, and refuse before `exact_in` if the buffer would exceed it. Done when an ordered response larger than the native cap fails on the web without retaining the bytes, and independent limits are unchanged.
