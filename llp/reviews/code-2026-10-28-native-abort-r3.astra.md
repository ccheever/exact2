# Code review: native abort (a fetch the app aborts is cancelled on every target), round 3 of 3, 2026-10-28 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `467d4ee011`.
- **Method:** one brief (sha256 `0ed929d0c939aa2bb0a48c6542dcfad801640e77929dfbea90c071fecb090488`), blind to the other reviewer; 3 rounds (runtime on a client-reachable path, theme2's 14:49 approval), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

Static review of **`1df7ab7829..467d4ee011`**. No builds or tests run. I found five issues, including three blockers.

1. **blocker — Replacing a fetch within the same source call loses its abort.**  
   [js/src/lib.rs:768](js/src/lib.rs:768) resolves abort tickets using the current parked entries. But a new fetch with the same target, source and arguments removes the previous entry at [lib.rs:1095](js/src/lib.rs:1095), before the runner drains aborts at [queue.rs:411](runner/src/runner/queue.rs:411).

   A source implementing `previousController.abort(); … return fetch(...)` therefore loses the old ticket’s ownership mapping when called again with equal arguments. An old POST continues physically; ordinary forget deliberately lets writes run. The new replacement test aborts through a separate send, which drains the abort before replacement and misses this case. Ownership must survive until the abort is collected.

2. **blocker — Linux and Android strand explicit cancellation of retired requests.**  
   [admission.rs:169](runner/src/runner/admission.rs:169) queues an explicit cancel for a retired ticket but returns no outcome. With no other completion, [presenter.rs:1362](host/linux/src/presenter.rs:1362) takes the empty-pump return, bypassing the cancellation drain at [presenter.rs:810](host/linux/src/presenter.rs:810).

   The abort has already left `module_aborts`, so it supplies no further wake. A held POST aborted and then replaced by a synchronous answer can consequently keep running indefinitely. Android uses this same pump at [android.rs:505](host/linux/src/android.rs:505). Explicit cancels need servicing even when there is no outcome to fulfill.

3. **blocker — Retirement aborts are retained without a wake on native workers and wasm realms.**  
   Native `Job::Forgotten` publishes aborts into the shared vector and immediately continues, without a reply or notification: [placed.rs:364](data/src/placed.rs:364). If that happens after the host’s final collection, nothing requests another collection. Render likewise waits for executor news at [render/src/lib.rs:849](host/render/src/lib.rs:849).

   Wasm has the same gap: the worker message handler only appends and returns at [module-glue.js:447](host/web/module-glue.js:447); the iframe door also only appends at [module-glue.js:135](host/web/module-glue.js:135). A worker forget has no completion response at [module-worker.js:252](host/web/module-worker.js:252).

   Concrete case: retiring call A runs its `finally`, which aborts still-active call B using `exactIndependentHttp`. If publication follows the last collection, B remains pending until unrelated activity or its transport replies. This is within the documented worker-placement boundary. Publishing an abort needs its own wake, independent of turn-result delivery.

4. **should-fix — Overlapping module ticket spaces can cancel the wrong request.**  
   [mixed.rs:565](data/src/mixed.rs:565) concatenates child aborts without namespacing. Request routing namespaces continuations, but leaves fetch tickets unchanged at [mixed.rs:294](data/src/mixed.rs:294). Matching then compares only target and fetch number at [admission.rs:117](runner/src/runner/admission.rs:117).

   For the overlapping-ticket configuration you asked about: module A produces and aborts fetch 1 for mutation T before enqueue; the commit subsequently refuses at [commit.rs:602](runner/src/runner/commit.rs:602). The unmatched abort survives rollback. Before expiry, T invokes module B’s fetch 1, which incorrectly matches A’s abort and is withheld. Aging limits the exposure window but does not establish identity. Fetch ownership needs a module namespace, including through composition and placement.

5. **should-fix — Runtime tool-handler fetches cannot deliver explicit aborts.**  
   Client callbacks run through `data.answer` at [invoke.rs:257](runner/src/runner/modules/invoke.rs:257), which starts the JS call with no target at [source.rs:375](js/src/source.rs:375). [lib.rs:769](js/src/lib.rs:769) then discards their aborts because `target` is `None`. Their requests also live in `invocations.running`, outside the matcher’s `pending` collection: [invoke.rs:351](runner/src/runner/modules/invoke.rs:351).

   Thus a payment-sheet or module callback awaiting a fetch cannot physically cancel it through its signal. The callback waits for transport completion despite the JS rejection. Targetless runtime invocations need cancellation ownership too.

The other reviewed areas look clean, subject to those delivery gaps:

- **Apple, web and render synchronous servicing:** Apple emits even after an empty pump ([abi.rs:390](host/apple/src/abi.rs:390)); web collects, fulfills and loops ([host.rs:1029](host/web/src/host.rs:1029)); render drains explicit cancels ([lib.rs:803](host/render/src/lib.rs:803)).
- **Single-module identity and normal reload:** fetch numbers increase within an engine; unload clears door-22 tickets, and a fresh runner clears all abort queues ([lib.rs:708](js/src/lib.rs:708), [runner.rs:864](runner/src/runner.rs:864)). I found no normal reload alias.
- **Withholding in `take_requests`:** no additional ordered-fence, held-continuation, timer or refusal defect found. Matching excludes continuations and streams, while the pending HTTP ticket remains available for synthetic settlement ([admission.rs:117](runner/src/runner/admission.rs:117), [commit.rs:948](runner/src/runner/commit.rs:948)).
- **Cancelling retired/completed tickets:** no harmful repeat cancellation found. Executor terminal claims guard settlement; web lookup is harmless when absent and cleanup checks controller identity ([executor_async.rs:1282](host/apple/src/executor_async.rs:1282), [glue.js:714](host/web/glue.js:714), [glue.js:760](host/web/glue.js:760)).
- **DNS helpers, bounded panic test and compatibility bump:** no additional defect found in those changes.
