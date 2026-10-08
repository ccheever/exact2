# Code review: exactBodyFrom (59f55d1d0), astra

Reviewer: GPT-6 Astra (codex, max reasoning, read-only), 2026-10-08, on the detached review worktree at 59f55d1d0.

Reviewed `59f55d1d0`. Findings, most severe first:

1. **[P1] Older native hosts silently send an empty body.**  
   [js/src/prelude.js:794](/private/tmp/bf-astra/js/src/prelude.js:794), [js/bake/src/lib.rs:898](/private/tmp/bf-astra/js/bake/src/lib.rs:898). New modules still declare ABI 1 or 2, which older hosts accept. Their embedded prelude ignores `exactBodyFrom`. An updated upload action therefore sends an empty POST on an existing installation. I reproduced this using the previous and current preludes in isolated JavaScript contexts.  
   **Fix:** require a newer JavaScript runtime ABI in generated modules/receipts and delivery requirements, so older hosts refuse activation. Add an old-host/new-module compatibility test.

2. **[P2] Native uploads reopen root paths instead of retaining storage’s directory handles.**  
   [host/apple/src/executor_body.rs:76](/private/tmp/bf-astra/host/apple/src/executor_body.rs:76). `AppDirectories::new` runs for every upload; its Unix root opens follow symlinks. If another local process renames an app root and replaces it with a symlink to another app’s directory after storage initialization, Hermes `readFile` retains the original directory, but `exactBodyFrom` reads the replacement under the original logical grant. Descendant `O_NOFOLLOW` checks do not protect this root lookup.  
   **Fix:** share the configured, pinned `AppDirectories` handles with the executor. Test root replacement after initialization.

3. **[P2] File loading sits outside the timeout and, on the JS target, abort handling.**  
   [host/web-js/ts-fetch.js:31](/private/tmp/bf-astra/host/web-js/ts-fetch.js:31), [host/web/http-body.js:184](/private/tmp/bf-astra/host/web/http-body.js:184), [host/apple/src/executor_core.rs:914](/private/tmp/bf-astra/host/apple/src/executor_core.rs:914). All three start the deadline after loading the file. The JS target also waits for that load before observing the caller’s signal. A stalled storage read can therefore exceed `exactTimeout` indefinitely or leave an aborted fetch pending. An in-memory probe with a 60 ms loader and 5 ms timeout sent successfully after 64 ms; another remained pending after abort until the loader finished.  
   **Fix:** establish cancellation/deadline handling before file acquisition, prevent sending after expiry, and test delayed reads and aborts.

4. **[P2] The non-Unix fallback has an uncapped read.**  
   [host/apple/src/executor_body.rs:95](/private/tmp/bf-astra/host/apple/src/executor_body.rs:95). On Windows, `FsOp::ReadFile` ultimately uses `read_to_end`; the size check follows allocation. A file replaced or grown beyond 64 MiB between `Stat` and the read can consume arbitrarily more memory before rejection.  
   **Fix:** implement a capped read through the opened Windows handle, or explicitly refuse this option there until available. Test growth/replacement between inspection and reading.

5. **[P2] Browser uploads copy oversized byte entries before refusing them.**  
   [host/web/storage-fs.js:418](/private/tmp/bf-astra/host/web/storage-fs.js:418). `store.blob(normalized, '')` uses the default unlimited ceiling and constructs a Blob before `requestBody` checks its size. An oversized ArrayBuffer entry incurs another full copy and can exhaust memory instead of delivering the promised refusal. My probe confirmed a 67,108,865-byte Blob construction before rejection.  
   **Fix:** check the record’s size before Blob construction, using the helper’s existing cap argument and upload-specific diagnostics. IndexedDB’s whole-record read remains a separate documented limitation.

6. **[P2] The new path buffer escapes native request memory accounting.**  
   [host/apple/src/executor_core.rs:678](/private/tmp/bf-astra/host/apple/src/executor_core.rs:678). `reservation()` counts the other request buffers but omits `body_from.capacity()`. Requests containing multi-megabyte paths can pass the 4 MiB request limit and accumulate uncharged memory while queued. This is request metadata, distinct from the intentionally unreserved file bytes acquired during execution.  
   **Fix:** include the path’s capacity in admission accounting; test oversized paths and retained capacity.

7. **[P2] WebSocket streams silently discard `exactBodyFrom`.**  
   [host/apple/src/executor_stream.rs:66](/private/tmp/bf-astra/host/apple/src/executor_stream.rs:66), [host/web/http-body.js:169](/private/tmp/bf-astra/host/web/http-body.js:169). A POST-shaped `exactStream` request to `wss:` passes validation, then takes the socket branch before file resolution. Even a missing or ungranted path opens a connection without reporting the file problem. The browser helper probe confirmed socket creation with zero file-reader calls.  
   **Fix:** explicitly reject `exactBodyFrom` for WebSocket requests before opening the connection, across the preludes and host admission paths.

8. **[P3] Missing filesystem grants can produce an adapter-loading error instead of the required denial.**  
   [host/web-js/ts-fetch.js:22](/private/tmp/bf-astra/host/web-js/ts-fetch.js:22). The JS target imports storage before checking `fs.read`. Builds without filesystem usage/grants can omit that adapter, so the rejection names a failed dynamic import instead of the requested path and missing grant. The wasm HTTP helper already checks admission before loading adapters.  
   **Fix:** perform the same grant check before importing storage; test a built app with only network grants.

Skipping the **Rust logic ABI bump is correct**: its encoder explicitly refuses `body_from`. No plan-format change is inherently needed. Skipping a **JavaScript runtime compatibility requirement is incorrect**, for finding 1.

No files changed. Verification used source inspection and in-memory probes; no builds or repository test suites were run.

**BLOCKS landing: YES — findings 1–7. Finding 8 is nonblocking.**