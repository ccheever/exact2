I found three **should-fix** issues.

1. **Empty binary bodies still bypass GET/HEAD rejection.** [host/web/module-glue.js:54](/private/tmp/bsky4-rv/wt-binary/host/web/module-glue.js:54)  
   `fetch(url, {body: new ArrayBuffer(0)})` produces `body: ""` and `body_base64: ""`. Both are falsy, so the early path issues a bodyless GET. The later transports also collapse empty bytes into an absent body, affecting HEAD too. [Fetch requires rejection for any supplied GET/HEAD body, including an empty one](https://fetch.spec.whatwg.org/#dom-request). Reject these requests in the prelude before calling the host, and make the early guard check field presence. Add empty and nonempty buffer/view tests asserting zero requests.

2. **The detached-buffer fix contradicts WHATWG extraction.** [js/src/prelude.js:776](/private/tmp/bsky4-rv/wt-binary/js/src/prelude.js:776)  
   For `const b = new ArrayBuffer(1); structuredClone(b, {transfer: [b]});`, a subsequent POST with `body: b` rejects because `copyBytes` calls `.slice()`. However, [Web IDL’s buffer-copy algorithm returns empty bytes for detached buffers](https://webidl.spec.whatwg.org/#dfn-get-buffer-source-copy). This creates a difference between the wasm prelude and the browser-backed JS target. Implement Fetch’s detached-buffer extraction and test detached buffers, typed arrays, and DataViews against the browser.

3. **BufferSource validation accepts backing stores that Fetch excludes.** [js/src/prelude.js:777](/private/tmp/bsky4-rv/wt-binary/js/src/prelude.js:777)  
   A POST using `new Uint8Array(new ArrayBuffer(3, {maxByteLength: 6}))` is copied into a fixed buffer and sent by the prelude; the JS target’s browser Fetch rejects the resizable input. SharedArrayBuffer-backed views also pass this check despite Fetch’s unannotated BufferSource type excluding them. [Web IDL defines these restrictions](https://webidl.spec.whatwg.org/#es-buffer-source-types). Validate the backing store before copying and add rejection tests. Bare SharedArrayBuffer stringification is a separate, valid union fallback.

The ordinary upload path looks sound: probes preserved offsets, byte lengths, high-byte values, and the snapshot before subsequent mutation. I traced the Apple iOS/macOS, Linux, wasm-page, and JS-target paths. Explicit Content-Type survives; Apple’s automatic Content-Type when none is supplied remains unverified, as do actual on-wire lengths.

The [native test](/private/tmp/bsky4-rv/wt-binary/js/tests/it/castle.rs:225) and [browser test](/private/tmp/bsky4-rv/wt-binary/js/web/tests/browser.rs:532) would catch the original stringification bug, but stop before transport and do not test either second-commit fix. Add transport echo coverage with and without Content-Type. The [reference’s binary-body claim](/private/tmp/bsky4-rv/wt-binary/docs/reference.md:372) does not disclose these deviations.

Read-only verification included baseline-versus-HEAD probes, six existing HTTP-helper tests, and `git diff --check`. No files changed; full builds and live transport tests were not run.

**I would hold landing until the three parity issues are resolved.**